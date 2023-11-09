// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use anyhow::{anyhow, bail, Context, Result};
use glib::WeakRef;
use gst::prelude::*;
use gst_webrtc::WebRTCPeerConnectionState;
use std::fmt::{Debug, Display};
use std::sync::Arc;
use tokio::sync::oneshot;

use crate::{log, Source};

/// Source that connects to an `WebRTC` source and provides the incoming streams as participant's input.
#[derive(Debug)]
pub struct WebRtcSource {
    /// GStreamer bin surrounding all included elements
    bin: gst::Bin,
    /// WebRTC GStreamer element which manages mostly everything.
    webrtcbin: gst::Element,
    video_src: gst::GhostPad,
    audio_src: gst::GhostPad,
}

type OnCandidateCallback = Arc<dyn Fn(u32, Option<String>) + Send + Sync>;

#[derive(Default)]
pub struct WebRtcSourceParams {
    on_ice_candidate: Option<OnCandidateCallback>,
}

impl Debug for WebRtcSourceParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebRtcSourceParams").finish()
    }
}

impl WebRtcSourceParams {
    pub fn on_ice_candidate<F>(mut self, f: F) -> Self
    where
        F: Fn(u32, Option<String>) + Send + Sync + 'static,
    {
        self.on_ice_candidate = Some(Arc::new(f));
        self
    }
}

impl Source for WebRtcSource {
    type Parameters = WebRtcSourceParams;

    /// Create a new `WebRTC` source
    fn new<ID>(id: &ID, params: Self::Parameters) -> Self
    where
        ID: Display,
    {
        debug!("new( {id},_, {params:?} )");

        let bin = gst::parse_bin_from_description(
            r#"
            webrtcbin
                name=webrtc
                bundle-policy=max-bundle
                latency=1000
            "#,
            false,
        )
        .expect("Failed to parse and load WebRtc pipeline. Is a gst plugin missing?");

        let webrtcbin = bin
            .by_name("webrtc")
            .expect("failed to find webrtc in pipeline");

        let video_src = gst::GhostPad::new(Some("video"), gst::PadDirection::Src);
        let audio_src = gst::GhostPad::new(Some("audio"), gst::PadDirection::Src);

        bin.add_pad(&video_src)
            .expect("failed to add video output ghost pad to webrtc bin");
        bin.add_pad(&audio_src)
            .expect("failed to add audio output ghost pad to webrtc bin");

        webrtcbin.connect_pad_added(webrtcbin_on_pad_added(
            bin.downgrade(),
            audio_src.downgrade(),
            video_src.downgrade(),
        ));

        if let Some(on_candidate) = params.on_ice_candidate {
            let on_candidate1 = Arc::clone(&on_candidate);

            webrtcbin.connect("on-ice-candidate", true, move |values| {
                let mline_index = values[1].get::<u32>().expect("mline_index is guint");
                let candidate = values[2].get::<String>().expect("candidate is gchararray");

                on_candidate1(mline_index, Some(candidate));
                None
            });

            webrtcbin.connect_notify(Some("ice-gathering-state"), move |webrtcbin, _| {
                let state = webrtcbin
                    .property::<gst_webrtc::WebRTCICEGatheringState>("ice-gathering-state");

                if state == gst_webrtc::WebRTCICEGatheringState::Complete {
                    on_candidate(0, None) // TODO: Setting mline_index to 0 here because there's just no way to tell
                }
            });
        }

        Self {
            bin,
            webrtcbin,
            video_src,
            audio_src,
        }
    }

    fn bin(&self) -> gst::Bin {
        self.bin.clone()
    }

    fn video(&self) -> gst::GhostPad {
        self.video_src.clone()
    }

    fn audio(&self) -> gst::GhostPad {
        self.audio_src.clone()
    }

    fn is_video_connected(&self) -> bool {
        self.webrtcbin
            .property::<WebRTCPeerConnectionState>("connection-state")
            == WebRTCPeerConnectionState::Connected
    }
    fn is_audio_connected(&self) -> bool {
        self.webrtcbin
            .property::<WebRTCPeerConnectionState>("connection-state")
            == WebRTCPeerConnectionState::Connected
    }
}

impl WebRtcSource {
    pub async fn receive_offer(&self, offer: String) -> anyhow::Result<String> {
        trace!("receive_offer()");

        let sdp_offer = gst_sdp::SDPMessage::parse_buffer(offer.as_bytes())
            .with_context(|| format!("failed to parse webrtc offer {offer}"))?;

        let offer_description =
            gst_webrtc::WebRTCSessionDescription::new(gst_webrtc::WebRTCSDPType::Offer, sdp_offer);

        self.webrtcbin.emit_by_name::<()>(
            "set-remote-description",
            &[&offer_description, &None::<gst::Promise>],
        );

        let (send, recv) = oneshot::channel();

        let webrtcbin_weak = self.webrtcbin.downgrade();
        let on_create_answer = gst::Promise::with_change_func(
            move |answer: Result<Option<&gst::StructureRef>, gst::PromiseError>| {
                let Some(webrtcbin) = webrtcbin_weak.upgrade() else {
                    return;
                };

                let result = match answer {
                    Ok(Some(create_answer)) => create_answer
                        .get::<gst_webrtc::WebRTCSessionDescription>("answer")
                        .map(|local_description| {
                            webrtcbin.emit_by_name::<()>(
                                "set-local-description",
                                &[&local_description, &None::<gst::Promise>],
                            );

                            local_description.sdp().to_string()
                        })
                        .with_context(|| {
                            format!(
                                "webrtc session could not configure local_description for offer {offer}",
                            )
                        }),
                    Ok(None) => Err(anyhow!(
                        "failed to 'create-answer' for webrtc offer {} - empty",
                        offer
                    )),
                    Err(err) => Err(anyhow!(
                        "failed to 'create-answer' for webrtc offer {} - {:?}",
                        offer,
                        err
                    )),
                };

                if let Err(e) = send.send(result) {
                    error!("Failed to send SDP answer result {:?} to main webrtc thread. Receiver dropped?", e);
                };
            },
        );

        // Call create-answer
        self.webrtcbin.emit_by_name::<()>(
            "create-answer",
            &[&None::<gst::Structure>, &on_create_answer],
        );

        recv.await?
    }

    pub fn receive_candidate(&self, mline: u32, candidate: String) {
        trace!("receive_candidate()");

        self.webrtcbin
            .emit_by_name("add-ice-candidate", &[&mline, &candidate])
    }

    pub fn receive_end_of_candidates(&self, mline: u32) {
        trace!("receive_end_of_candidates()");

        self.webrtcbin
            .emit_by_name("add-ice-candidate", &[&mline, &None::<String>])
    }
}

/// Creates a closure which is called by webrtcbin when it added a new pad.
///
/// Pads are created for media stream negotiated using SDP.
fn webrtcbin_on_pad_added(
    bin: WeakRef<gst::Bin>,
    audio_ghost_pad: WeakRef<gst::GhostPad>,
    video_ghost_pad: WeakRef<gst::GhostPad>,
) -> impl Fn(&gst::Element, &gst::Pad) {
    move |_, pad| {
        let Some(bin) = bin.upgrade() else {
            return;
        };

        // Make sure this is a source pad
        if pad.direction() != gst::PadDirection::Src {
            log::error!("Got sink pad in subscriber webrtbin");
            return;
        }

        if let Err(e) = try_webrtcbin_on_pad_added(
            bin,
            pad.clone(),
            audio_ghost_pad.clone(),
            video_ghost_pad.clone(),
        ) {
            log::error!("Failed to handle webrtcbin's pad-added event, {e:?}",);
        }
    }
}

fn try_webrtcbin_on_pad_added(
    bin: gst::Bin,
    pad: gst::Pad,
    audio_ghost_pad: WeakRef<gst::GhostPad>,
    video_ghost_pad: WeakRef<gst::GhostPad>,
) -> Result<()> {
    // Check what kind of media this pad emits
    let caps = pad.caps().context("no caps in added pad")?;
    let caps = caps.structure(0).context("empty caps list")?;

    let media: String = caps
        .get("media")
        .context("Failed to get media type from rtp field")?;

    let name = match media.as_str() {
        "audio" => "audio-in",
        "video" => "video-in",
        _ => bail!("Got unknown media type {media} on new pad from webrtcbin"),
    };

    // Create a decodebin which will decode the rtp to raw media (audio/video)
    let decodebin = gst::ElementFactory::make("decodebin").name(name).build()?;

    // Handle new source pads created by decodebin
    decodebin.connect_pad_added(decodebin_on_pad_added(
        bin.downgrade(),
        audio_ghost_pad,
        video_ghost_pad,
    ));

    // Add the decodebin to the subscriber bin and sync its current running state
    bin.add(&decodebin)?;
    decodebin.sync_state_with_parent()?;

    // link the new pad with the decodebin
    let decode_sink_pad = decodebin
        .static_pad("sink")
        .expect("decodebin has a static_pad named `sink`");
    pad.link(&decode_sink_pad)?;

    Ok(())
}

/// Creates a closure which is called by gstreamer when the decodebin object added a new pad.
///
/// Pads are created for every media format decodebin recogizes on its sink
fn decodebin_on_pad_added(
    bin: WeakRef<gst::Bin>,
    audio_ghost_pad: WeakRef<gst::GhostPad>,
    video_ghost_pad: WeakRef<gst::GhostPad>,
) -> impl Fn(&gst::Element, &gst::Pad) {
    move |_, pad| {
        let Some(bin) = bin.upgrade() else { return };
        let Some(audio_ghost_pad) = audio_ghost_pad.upgrade() else {
            return;
        };
        let Some(video_ghost_pad) = video_ghost_pad.upgrade() else {
            return;
        };

        // Make sure this is a source pad
        if pad.direction() != gst::PadDirection::Src {
            log::error!("Got sink pad in subscriber webrtbin");
            return;
        }

        if let Err(e) = try_decodebin_on_pad_added(bin, pad, audio_ghost_pad, video_ghost_pad) {
            log::error!("Failed to handle decodebin's pad-added event, {e:?}");
        }
    }
}

fn try_decodebin_on_pad_added(
    bin: gst::Bin,
    pad: &gst::Pad,
    audio_ghost_pad: gst::GhostPad,
    video_ghost_pad: gst::GhostPad,
) -> Result<()> {
    // Check what kind of media this pad emits
    let caps = pad.caps().context("no caps in added pad")?;
    let caps = caps.structure(0).context("empty caps list")?;

    let (convert_name, scale_name, ghost_pad) = if caps.name().starts_with("audio") {
        ("audioconvert", "audioresample", audio_ghost_pad)
    } else if caps.name().starts_with("video") {
        ("videoconvert", "videoscale", video_ghost_pad)
    } else {
        // If it neither audio nor video we ignore it
        warn!(
            "Receiving unknown media media, expected either audio or video (got {})",
            caps.name()
        );
        return Ok(());
    };

    // Check if the ghost pad for the audio or video has already has it's target set.
    // If the target has been set it means we've received a second stream with the same media type (audio/video)
    if ghost_pad.target().is_some() {
        warn!("Received another {} stream in the same WebRTC subscription, discarding duplicate media types", caps.name());
        return Ok(());
    }

    let convert = gst::ElementFactory::make(convert_name).build()?;
    let scale = gst::ElementFactory::make(scale_name).build()?;
    let queue = gst::ElementFactory::make("queue").build()?;

    bin.add_many(&[&convert, &scale, &queue])?;
    gst::Element::link_many(&[&convert, &scale, &queue])?;

    let convert_sink = convert
        .static_pad("sink")
        .expect("convert element has a static pad `sink`");
    pad.link(&convert_sink)?;

    let queue_src = queue.static_pad("src").expect("queue has static pad `src`");

    ghost_pad.set_target(Some(&queue_src))?;

    convert.sync_state_with_parent()?;
    scale.sync_state_with_parent()?;
    queue.sync_state_with_parent()?;

    Ok(())
}
