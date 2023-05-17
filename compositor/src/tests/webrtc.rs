use crate::testing;
use crate::{Grid, Size, WebRtcSource, WebRtcSourceParams};
use crate::{StreamId, StreamStatus};
use core::time::Duration;
use glib::{Cast, Continue, ObjectExt};
use gst::prelude::*;
use gst::traits::{ElementExt, GstBinExt};
use std::collections::HashMap;
use tokio::sync::mpsc;
use tokio::time::sleep;

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

    const MAX_VISIBLES: Option<usize> = Some(7);

    let mut talk = Talk::new(
        Size::FHD,
        // Mp4SinkParams {
        //     file_path: "out.mp4".into(),
        // },
        Box::<testing::TestSinkBuilder>::default(),
        MAX_VISIBLES,
    )
    .unwrap();

    talk.layout::<Grid>().unwrap();

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
                    break;
                }
            }
            Some(event) = rx.recv() => {
                handle_webrtc_event(&mut talk, &mut participants, event).await
            }
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

            let source = &talk.source_mut(&StreamId::camera(id)).unwrap().source;
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
            let source = &talk.source_mut(&StreamId::camera(id)).unwrap().source;
            source.receive_candidate(mline, candidate).await;
        }
        WebRtcBinToMainLoopEvent::SdpEndOfCandidates(id) => {
            let source = &talk.source_mut(&StreamId::camera(id)).unwrap().source;
            source.receive_end_of_candidates(0).await;
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
            talk.layout::<Grid>().unwrap();
        }
        Event::Unpublish(id) => {
            log::debug!("Participant with id={id} stops publishing");
            let state = participants.get_mut(&id).unwrap();

            if let Some(publish) = state.publish.take() {
                publish.set_state(gst::State::Null).unwrap();
            }

            let id = crate::StreamId::new(id, crate::MediaSessionType::Camera);
            talk.remove_stream(id).unwrap();
            talk.layout::<Grid>().unwrap();
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
            videotestsrc is-live=true pattern=ball ! video/x-raw,width=720,height=480 ! vp8enc ! rtpvp8pay ! webrtc.
            audiotestsrc is-live=true volume=0.02 freq=300 ! opusenc ! rtpopuspay ! webrtc.
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

    let bus = pipeline.bus().unwrap();

    // ON LAST ICE CANDIDATE
    bus.add_watch_local({
        let tx = tx.clone();

        move |_, msg| {
            if let gst::MessageView::PropertyNotify(prop) = msg.view() {
                let (_obj, name, value) = prop.get();

                if name == "ice-gathering-state" {
                    if let Some(value) = value {
                        if let Ok(state) = value.get::<gst_webrtc::WebRTCICEGatheringState>() {
                            if state == gst_webrtc::WebRTCICEGatheringState::Complete {
                                let _ = tx.send(WebRtcBinToMainLoopEvent::SdpEndOfCandidates(id));
                            }
                        }
                    }
                }
            }

            Continue(true)
        }
    })
    .unwrap();

    // ON NEGOTIATION NEEDED
    webrtcbin.connect("on-negotiation-needed", true, {
        let webrtcbin = webrtcbin.clone();
        let tx = tx.clone();

        move |_| {
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

    talk.add_stream(
        StreamId::camera(id),
        &format!("Mock {id}"),
        WebRtcSourceParams::default().on_ice_candidate(move |mline, candidate| {
            if let Some(candidate) = candidate {
                webrtcbin.emit_by_name::<()>("add-ice-candidate", &[&mline, &candidate]);
            } else {
                webrtcbin.emit_by_name::<()>("add-ice-candidate", &[&mline, &None::<String>]);
            }
        }),
        StreamStatus {
            has_audio: true,
            has_video: true,
        },
    )
    .unwrap();
    talk.layout::<Grid>().unwrap();
}

// --- scenarios

#[tokio::test]
async fn scenario1() {
    let _ = env_logger::try_init();

    exec_events(vec![
        Event::AddParticipant(0),
        Event::Publish(0),
        Event::Sleep(Duration::from_secs(10)),
        Event::Unpublish(0),
        Event::RemoveParticipant(0),
    ])
    .await;
}
