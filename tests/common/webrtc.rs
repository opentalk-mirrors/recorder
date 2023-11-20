// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use compositor::{debug, MediaSessionType};
use gst::{
    prelude::*,
    traits::{ElementExt, GstBinExt},
};
use opentalk_recorder::signaling::{
    incoming::{MediaMessage, Message, Sdp, SdpCandidate, Source},
    ParticipantId, TrickleCandidate,
};
use tokio::sync::mpsc;
use uuid::Uuid;

pub(crate) fn create_pipeline(
    uuid: Uuid,
    media_session_type: MediaSessionType,
    to_recorder_tx: mpsc::Sender<Message>,
) -> gst::Pipeline {
    let pattern = match media_session_type {
        MediaSessionType::Camera => "ball",
        MediaSessionType::ScreenCapture => "smpte",
    };
    let audiotestsrc = match media_session_type {
        MediaSessionType::Camera => {
            "audiotestsrc is-live=true volume=0.02 freq=300 ! opusenc ! rtpopuspay pt=100 ! webrtc."
        }
        MediaSessionType::ScreenCapture => "",
    };
    let pipeline = gst::parse_launch(format!(r#"
        webrtcbin name=webrtc bundle-policy=max-bundle
        videotestsrc is-live=true pattern={pattern} ! video/x-raw,width=720,height=480 ! vp8enc deadline=1 ! rtpvp8pay pt=101 ! webrtc.
        {audiotestsrc}
    "#).as_str(),)
    .unwrap()
    .downcast::<gst::Pipeline>()
    .unwrap();

    let webrtcbin = pipeline.by_name("webrtc").unwrap();
    //webrtcbin.add_property_notify_watch(Some("ice-gathering-state"), true);

    // ON ICE CANDIDATE
    webrtcbin.connect("on-ice-candidate", true, {
        let to_recorder_tx = to_recorder_tx.clone();

        move |values| {
            let sdp_m_line_index = u64::from(values[1].get::<u32>().expect("mline_index is guint"));
            let candidate = values[2].get::<String>().expect("candidate is gchararray");

            to_recorder_tx
                .blocking_send(Message::Media(MediaMessage::SdpCandidate(SdpCandidate {
                    candidate: TrickleCandidate {
                        candidate,
                        sdp_m_line_index,
                    },
                    source: Source {
                        source: ParticipantId(uuid),
                        media_session_type,
                    },
                })))
                .unwrap();

            None
        }
    });

    // ON LAST ICE CANDIDATE
    webrtcbin.connect_notify(Some("ice-gathering-state"), {
        let to_recorder_tx = to_recorder_tx.clone();

        move |webrtcbin, _| {
            let state =
                webrtcbin.property::<gst_webrtc::WebRTCICEGatheringState>("ice-gathering-state");

            if state == gst_webrtc::WebRTCICEGatheringState::Complete {
                to_recorder_tx
                    .blocking_send(Message::Media(MediaMessage::SdpEndOfCandidates(Source {
                        source: ParticipantId(uuid),
                        media_session_type,
                    })))
                    .unwrap();
            }
        }
    });

    // ON NEGOTIATION NEEDED
    webrtcbin.connect("on-negotiation-needed", true, {
        let webrtcbin_weak = webrtcbin.downgrade();
        let to_recorder_tx = to_recorder_tx.clone();

        move |_| {
            let webrtcbin = webrtcbin_weak.upgrade()?;

            let on_create_offer = {
                // Clone webrtcbin and tx once to move it into the Promise
                let to_recorder_tx = to_recorder_tx.clone();
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

                    to_recorder_tx
                        .blocking_send(Message::Media(MediaMessage::SdpOffer(Sdp {
                            sdp: offer.sdp().to_string(),
                            source: Source {
                                source: ParticipantId(uuid),
                                media_session_type,
                            },
                        })))
                        .unwrap();
                })
            };

            webrtcbin
                .emit_by_name::<()>("create-offer", &[&None::<gst::Structure>, &on_create_offer]);

            None
        }
    });

    pipeline.set_state(gst::State::Playing).unwrap();

    debug::debug_dot(&pipeline, "webrtcbin");

    pipeline
}
