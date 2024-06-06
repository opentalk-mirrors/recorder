// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use core::time::Duration;
use std::collections::HashMap;

use glib::{Cast, Continue, ObjectExt};
use gst::{
    prelude::*,
    traits::{ElementExt, GstBinExt},
};
use tokio::{sync::mpsc, time::sleep};
use types::{
    core::ParticipantId,
    signaling::media::{MediaSessionState, MediaSessionType},
};

use crate::{
    log, GstBinErrorExt, GstElementErrorExt, MediaDescriptor, Mixer, Size, Speaker, TestSink,
    WebRtcSource, WebRtcSourceParams,
};

#[derive(Debug, Clone, Copy)]
enum Event {
    /// Wait the specified duration before handling the next events in the list
    Sleep(Duration),

    /// Simulate a participant joining
    AddParticipant(ParticipantId),

    /// Simulate a participant leaving
    RemoveParticipant(ParticipantId),

    /// Simulate a participant publishing it's camera/webcam
    Publish(ParticipantId),

    /// Simulate a participant unpublishing it's camera/webcam
    Unpublish(ParticipantId),
}

#[derive(Default)]
struct MockParticipantState {
    publish: Option<gst::Pipeline>,
}

#[derive(Debug)]
#[allow(clippy::enum_variant_names)]
enum WebRtcBinToMainLoopEvent {
    SdpOffer(ParticipantId, String),
    SdpCandidate(ParticipantId, u32, String),
    SdpEndOfCandidates(ParticipantId),
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

    let mut mixer = Mixer::<WebRtcSource>::create(
        None,
        Size::FHD,
        Speaker::default(),
        MAX_VISIBLES,
        true,
        &Default::default(),
    )
    .unwrap();

    mixer
        .link_sink("test_sink", TestSink::create("Recording", true).unwrap())
        .unwrap();

    let (tx, mut rx) = mpsc::unbounded_channel();

    let mut participants = HashMap::<ParticipantId, MockParticipantState>::new();

    let mut sleep_future = Box::pin(sleep(Duration::from_secs(0)));

    loop {
        tokio::select! {
            _ = &mut sleep_future => {
                if let Some(event) = events.next() {
                    if let Event::Sleep(dur) = event {
                        sleep_future = Box::pin(sleep(dur));
                    } else {
                        handle_user_event(event, &mut participants, &tx, &mut mixer);
                    }
                } else {
                    main_loop.quit();
                    break;
                }
            }
            Some(event) = rx.recv() => {
                handle_webrtc_event(&mut mixer, &mut participants, event).await
            }
        }
    }

    for (_, participant) in participants {
        if let Some(pipeline) = participant.publish {
            pipeline.set_state_with_context(gst::State::Null).unwrap();
        }
    }
}

async fn handle_webrtc_event(
    mixer: &mut Mixer<WebRtcSource>,
    participants: &mut HashMap<ParticipantId, MockParticipantState>,
    event: WebRtcBinToMainLoopEvent,
) {
    match event {
        WebRtcBinToMainLoopEvent::SdpOffer(id, offer) => {
            let publish = participants
                .get_mut(&id)
                .and_then(|p| p.publish.as_mut())
                .unwrap();

            let source = &mixer
                .stream_mut(MediaDescriptor {
                    participant_id: id,
                    media_type: MediaSessionType::Video,
                })
                .unwrap()
                .source;
            let webrtcbin = publish.by_name_with_context("webrtc").unwrap();
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
            let source = &mixer
                .stream_mut(MediaDescriptor {
                    participant_id: id,
                    media_type: MediaSessionType::Video,
                })
                .unwrap()
                .source;
            source.receive_candidate(mline, &candidate);
        }
        WebRtcBinToMainLoopEvent::SdpEndOfCandidates(id) => {
            let source = &mixer
                .stream_mut(MediaDescriptor {
                    participant_id: id,
                    media_type: MediaSessionType::Video,
                })
                .unwrap()
                .source;
            source.receive_end_of_candidates(0);
        }
    }
}

fn handle_user_event(
    event: Event,
    participants: &mut HashMap<ParticipantId, MockParticipantState>,
    tx: &mpsc::UnboundedSender<WebRtcBinToMainLoopEvent>,
    mixer: &mut Mixer<WebRtcSource>,
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
                screen.set_state_with_context(gst::State::Null).unwrap();
            }
        }
        Event::Publish(id) => {
            log::debug!("Participant with id={id} starts publishing");

            let state = participants.get_mut(&id).unwrap();
            assert!(state.publish.is_none());

            create_publish_pipeline(tx, id, state, mixer);
            mixer
                .show_stream(MediaDescriptor {
                    participant_id: id,
                    media_type: MediaSessionType::Video,
                })
                .unwrap();
        }
        Event::Unpublish(id) => {
            log::debug!("Participant with id={id} stops publishing");
            let state = participants.get_mut(&id).unwrap();

            if let Some(publish) = state.publish.take() {
                publish.set_state_with_context(gst::State::Null).unwrap();
            }

            let media_id = MediaDescriptor {
                participant_id: id,
                media_type: MediaSessionType::Video,
            };
            mixer.remove_stream(media_id).unwrap();
        }
    }
}

fn create_publish_pipeline(
    tx: &mpsc::UnboundedSender<WebRtcBinToMainLoopEvent>,
    id: ParticipantId,
    state: &mut MockParticipantState,
    mixer: &mut Mixer<WebRtcSource>,
) {
    let pipeline = gst::parse_launch(
        r#"
            webrtcbin name=webrtc bundle-policy=max-bundle
            videotestsrc is-live=true pattern=ball ! video/x-raw,width=720,height=480 ! vp8enc ! rtpvp8pay pt=100 ! webrtc.
            audiotestsrc is-live=true volume=0.02 freq=300 ! opusenc ! rtpopuspay pt=101 ! webrtc.
        "#,
    )
    .unwrap()
    .downcast::<gst::Pipeline>()
    .unwrap();

    let webrtcbin = pipeline.by_name_with_context("webrtc").unwrap();
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

    pipeline
        .set_state_with_context(gst::State::Playing)
        .unwrap();
    state.publish = Some(pipeline);

    let webrtcbin_weak = webrtcbin.downgrade();
    mixer
        .add_stream(
            MediaDescriptor {
                participant_id: id,
                media_type: MediaSessionType::Video,
            },
            format!("Mock {id}"),
            WebRtcSourceParams::new(true).on_ice_candidate(move |mline, candidate| {
                if let Some(webrtcbin) = webrtcbin_weak.upgrade() {
                    webrtcbin.emit_by_name::<()>("add-ice-candidate", &[&mline, &candidate]);
                }
            }),
            MediaSessionState {
                audio: true,
                video: true,
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
        Event::AddParticipant(ParticipantId::from_u128(0)),
        Event::Publish(ParticipantId::from_u128(0)),
        Event::Sleep(Duration::from_secs(10)),
        Event::Unpublish(ParticipantId::from_u128(0)),
        Event::RemoveParticipant(ParticipantId::from_u128(0)),
        Event::Sleep(Duration::from_secs(10)),
    ])
    .await;
}
