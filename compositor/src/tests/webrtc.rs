// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use core::time::Duration;
use glib::{Cast, Continue, ObjectExt};
use gst::{
    prelude::*,
    traits::{ElementExt, GstBinExt},
};
use std::collections::HashMap;
use tokio::{sync::mpsc, time::sleep};

use crate::{
    log, Size, Speaker, StreamId, StreamStatus, TestSink, WebRtcSource, WebRtcSourceParams,
};

type Talk = crate::Talk<WebRtcSource, usize>;

#[derive(Debug, Clone, Copy)]
enum Event {
    /// Wait the specified duration before handling the next events in the list
    Sleep(Duration),

    /// Simulate a participant joining
    AddParticipant(usize),

    /// Simulate a participant leaving
    RemoveParticipant(usize),

    /// Simulate a participant publishing it's camera/webcam
    Publish(usize),

    /// Simulate a participant unpublishing it's camera/webcam
    Unpublish(usize),
}

#[derive(Default)]
struct MockParticipantState {
    publish: Option<gst::Pipeline>,
}

#[derive(Debug)]
#[allow(clippy::enum_variant_names)]
enum WebRtcBinToMainLoopEvent {
    SdpOffer(usize, String),
    SdpCandidate(usize, u32, String),
    SdpEndOfCandidates(usize),
}

async fn exec_events(events: Vec<Event>) {
    let mut events = events.into_iter();

    gst::init().unwrap();

    // Run a MainLoop on a separate thread so gstreamer bus watches work
    let main_loop = glib::MainLoop::new(None, false);
    std::thread::spawn({
        let main_loop = main_loop.clone();

        move || {
            main_loop.run();
        }
    });

    const MAX_VISIBLES: usize = 7;

    let mut talk = Talk::new(Size::FHD, Speaker::default(), MAX_VISIBLES, true).unwrap();

    talk.link_sink("test_sink", TestSink::create("Recording", true).unwrap())
        .unwrap();

    let (tx, mut rx) = mpsc::unbounded_channel();

    let mut participants = HashMap::<usize, MockParticipantState>::new();

    let mut sleep_future = Box::pin(sleep(Duration::from_secs(0)));

    loop {
        tokio::select! {
            _ = &mut sleep_future => {
                if let Some(event) = events.next() {
                    if let Event::Sleep(dur) = event {
                        sleep_future = Box::pin(sleep(dur));
                    } else {
                        handle_user_event(event, &mut participants, &tx, &mut talk);
                    }
                } else {
                    main_loop.quit();
                    break;
                }
            }
            Some(event) = rx.recv() => {
                handle_webrtc_event(&mut talk, &mut participants, event).await
            }
        }
    }

    for (_, participant) in participants {
        if let Some(pipeline) = participant.publish {
            pipeline.set_state(gst::State::Null).unwrap();
        }
    }
}

async fn handle_webrtc_event(
    talk: &mut Talk,
    participants: &mut HashMap<usize, MockParticipantState>,
    event: WebRtcBinToMainLoopEvent,
) {
    match event {
        WebRtcBinToMainLoopEvent::SdpOffer(id, offer) => {
            let publish = participants
                .get_mut(&id)
                .and_then(|p| p.publish.as_mut())
                .unwrap();

            let source = &talk.stream_mut(&StreamId::camera(id)).unwrap().source;
            let webrtcbin = publish.by_name("webrtc").unwrap();
            let response = source.receive_offer(offer).await.unwrap();
            let response = gst_webrtc::WebRTCSessionDescription::new(
                gst_webrtc::WebRTCSDPType::Answer,
                gst_sdp::SDPMessage::parse_buffer(response.as_bytes()).unwrap(),
            );

            webrtcbin.emit_by_name::<()>(
                "set-remote-description",
                &[&response, &None::<gst::Promise>],
            );
        }
        WebRtcBinToMainLoopEvent::SdpCandidate(id, mline, candidate) => {
            let source = &talk.stream_mut(&StreamId::camera(id)).unwrap().source;
            source.receive_candidate(mline, &candidate);
        }
        WebRtcBinToMainLoopEvent::SdpEndOfCandidates(id) => {
            let source = &talk.stream_mut(&StreamId::camera(id)).unwrap().source;
            source.receive_end_of_candidates(0);
        }
    }
}

fn handle_user_event(
    event: Event,
    participants: &mut HashMap<usize, MockParticipantState>,
    tx: &mpsc::UnboundedSender<WebRtcBinToMainLoopEvent>,
    talk: &mut Talk,
) {
    match event {
        Event::Sleep(_) => unreachable!(),
        Event::AddParticipant(id) => {
            log::debug!("Adding participant with id={id}");

            assert!(participants
                .insert(id, MockParticipantState::default())
                .is_none());
        }
        Event::RemoveParticipant(id) => {
            log::debug!("Removing participant with id={id}");

            let mut state = participants.remove(&id).unwrap();
            if let Some(screen) = state.publish.take() {
                screen.set_state(gst::State::Null).unwrap();
            }
        }
        Event::Publish(id) => {
            log::debug!("Participant with id={id} starts publishing");

            let state = participants.get_mut(&id).unwrap();
            assert!(state.publish.is_none());

            create_publish_pipeline(tx, id, state, talk);
            talk.show_stream(&StreamId::camera(id)).unwrap();
        }
        Event::Unpublish(id) => {
            log::debug!("Participant with id={id} stops publishing");
            let state = participants.get_mut(&id).unwrap();

            if let Some(publish) = state.publish.take() {
                publish.set_state(gst::State::Null).unwrap();
            }

            let id = StreamId::new(id, crate::MediaSessionType::Camera);
            talk.remove_stream(id).unwrap();
        }
    }
}

fn create_publish_pipeline(
    tx: &mpsc::UnboundedSender<WebRtcBinToMainLoopEvent>,
    id: usize,
    state: &mut MockParticipantState,
    talk: &mut Talk,
) {
    let pipeline = gst::parse_launch(
        r#"
            webrtcbin name=webrtc bundle-policy=max-bundle latency=500
            videotestsrc is-live=true pattern=ball ! video/x-raw,width=720,height=480 ! vp8enc ! rtpvp8pay pt=100 ! webrtc.
            audiotestsrc is-live=true volume=0.02 freq=300 ! opusenc ! rtpopuspay pt=101 ! webrtc.
        "#,
    )
    .unwrap()
    .downcast::<gst::Pipeline>()
    .unwrap();

    let webrtcbin = pipeline.by_name("webrtc").unwrap();
    webrtcbin.add_property_notify_watch(Some("ice-gathering-state"), true);

    // ON ICE CANDIDATE
    webrtcbin.connect("on-ice-candidate", true, {
        let tx = tx.clone();

        move |values| {
            let mline = values[1].get::<u32>().expect("mline_index is guint");
            let candidate = values[2].get::<String>().expect("candidate is gchararray");

            let _ = tx.send(WebRtcBinToMainLoopEvent::SdpCandidate(id, mline, candidate));

            None
        }
    });

    // ON ICE GATHER STATE CHANGED
    webrtcbin.connect_notify(Some("ice-gathering-state"), {
        let tx = tx.clone();

        move |webrtcbin, _| {
            let state =
                webrtcbin.property::<gst_webrtc::WebRTCICEGatheringState>("ice-gathering-state");

            if state == gst_webrtc::WebRTCICEGatheringState::Complete {
                let _ = tx.send(WebRtcBinToMainLoopEvent::SdpEndOfCandidates(id));
            }
        }
    });

    let bus = pipeline.bus().unwrap();

    // ON LAST ICE CANDIDATE
    let pipeline_weak = pipeline.downgrade();
    bus.add_watch(move |_, msg| {
        if let gst::MessageView::Latency(_) = msg.view() {
            if let Some(pipeline) = pipeline_weak.upgrade() {
                let _ = pipeline.recalculate_latency();
            }
        }

        Continue(true)
    })
    .unwrap();

    // ON NEGOTIATION NEEDED
    webrtcbin.connect("on-negotiation-needed", true, {
        let webrtcbin_weak = webrtcbin.downgrade();
        let tx = tx.clone();

        move |_| {
            let webrtcbin = webrtcbin_weak.upgrade()?;

            let on_create_offer = {
                // Clone webrtcbin and tx once to move it into the Promise
                let tx = tx.clone();
                let webrtcbin = webrtcbin.clone();

                gst::Promise::with_change_func(move |offer| {
                    //  ON CREATE OFFER CALLBACK

                    // Get the (just created) SDP offer
                    let offer = offer
                        .unwrap()
                        .unwrap()
                        .get::<gst_webrtc::WebRTCSessionDescription>("offer")
                        .unwrap();

                    //  Set the offer as local description
                    webrtcbin.emit_by_name::<()>(
                        "set-local-description",
                        &[&offer, &None::<gst::Promise>],
                    );

                    // Send SDP offer to the signaling task
                    tx.send(WebRtcBinToMainLoopEvent::SdpOffer(
                        id,
                        offer.sdp().to_string(),
                    ))
                    .unwrap();
                })
            };

            webrtcbin
                .emit_by_name::<()>("create-offer", &[&None::<gst::Structure>, &on_create_offer]);

            None
        }
    });

    pipeline.set_state(gst::State::Playing).unwrap();
    state.publish = Some(pipeline);

    let webrtcbin_weak = webrtcbin.downgrade();
    talk.add_stream(
        StreamId::camera(id),
        &format!("Mock {id}"),
        WebRtcSourceParams::new(true).on_ice_candidate(move |mline, candidate| {
            if let Some(webrtcbin) = webrtcbin_weak.upgrade() {
                webrtcbin.emit_by_name::<()>("add-ice-candidate", &[&mline, &candidate]);
            }
        }),
        StreamStatus {
            has_audio: true,
            has_video: true,
        },
    )
    .unwrap();
}

// --- scenarios

#[tokio::test]
#[ignore = "failing in ci"]
async fn webrtc_scenario1() {
    let _ = env_logger::try_init();

    exec_events(vec![
        Event::AddParticipant(0),
        Event::Publish(0),
        Event::Sleep(Duration::from_secs(10)),
        Event::Unpublish(0),
        Event::RemoveParticipant(0),
        Event::Sleep(Duration::from_secs(10)),
    ])
    .await;
}
