use crate::*;

use gst::prelude::*;
use gst_webrtc::WebRTCPeerConnectionState;
use std::fmt::{Debug, Display};
use tokio::sync::oneshot;

/// Source that connects to an WebRTC source and provides the incoming streams as participant's input.
pub struct WebRtcSource {
    /// GStreamer bin surrounding all included elements
    bin: gst::Bin,
    /// WebRTC GStreamer element which manages mostly everything.
    webrtcbin: gst::Element,
    video_out_pad: gst::GhostPad,
    audio_out_pad: gst::GhostPad,
    /// Source overlays.
    overlays: Overlays,
}

type OnCandidateCallback = Box<dyn Fn(u32, Option<String>) + Send + Sync>;

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
        self.on_ice_candidate = Some(Box::new(f));
        self
    }
}

impl Source for WebRtcSource {
    type Parameters = WebRtcSourceParams;

    /// Create a new WebRTC source
    fn new<ID>(id: &ID, pipeline: &gst::Pipeline, _: &Size, params: Self::Parameters) -> Self
    where
        ID: Display,
    {
        debug!("new( {id},_, {params:?} )");

        let bin = gst::parse_bin_from_description(
            r#"
            webrtcbin
                name=webrtc
                bundle-policy=max-bundle

            webrtc.
            ! rtpvp8depay
                name=video-in
                request-keyframe=true
                wait-for-keyframe=true
            ! identity
                sync=true
            ! avdec_vp8
            ! videoconvert
            ! valve
                name=valve-overlay
            ! queue
                name=video-out

            webrtc.
            ! rtpopusdepay
                name=audio-in
            ! identity
                sync=true
            ! opusdec
            ! audioconvert
            ! queue
                name=audio-out
            "#,
            false,
        )
        .expect("Failed to parse and load WebRtc pipeline. Is a gst plugin missing?");

        pipeline
            .add(&bin)
            .expect("failed to add WebRtc bin to pipeline");

        let webrtcbin = bin
            .by_name("webrtc")
            .expect("failed to find webrtc in pipeline");

        let video_out = bin
            .by_name("video-out")
            .expect("failed to find webrtc video-out in pipeline");
        let video_out_src = video_out
            .static_pad("src")
            .expect("failed to get static source pad from webrtc video out");

        let audio_out = bin
            .by_name("audio-out")
            .expect("failed to find webrtc audio-out in pipeline");
        let audio_out_src = audio_out
            .static_pad("src")
            .expect("failed to get static source pad from webrtc audio output");

        let video_ghostpad = gst::GhostPad::with_target(Some("video"), &video_out_src)
            .expect("failed to create ghost pad for webrtc video output");
        let audio_ghostpad = gst::GhostPad::with_target(Some("audio"), &audio_out_src)
            .expect("failed to create ghost pad for webrtc audio output");

        bin.add_pad(&video_ghostpad)
            .expect("failed to add video output ghost pad to webrtc bin");
        bin.add_pad(&audio_ghostpad)
            .expect("failed to add audio output ghost pad to webrtc bin");

        if let Some(on_candidate) = params.on_ice_candidate {
            webrtcbin.connect("on-ice-candidate", true, move |values| {
                let mline_index = values[1].get::<u32>().expect("mline_index is guint");
                let candidate = values[2].get::<String>().expect("candidate is gchararray");

                on_candidate(mline_index, Some(candidate));

                None
            });
        }

        // create new overlays container
        let valve_overlay = pipeline
            .by_name("valve-overlay")
            .expect("failed to get video output valve from pipeline");
        let overlays = Overlays::new(valve_overlay);

        Self {
            bin,
            webrtcbin,
            video_out_pad: video_ghostpad,
            audio_out_pad: audio_ghostpad,
            overlays,
        }
    }

    fn video_inp_pad(&self) -> Option<gst::Pad> {
        if let Some(video_in) = self.bin.by_name("video-in") {
            if let Some(pad) = video_in.static_pad("sink") {
                return pad.peer();
            }
        }
        None
    }

    fn audio_inp_pad(&self) -> Option<gst::Pad> {
        if let Some(audio_in) = self.bin.by_name("audio-in") {
            if let Some(pad) = audio_in.static_pad("sink") {
                return pad.peer();
            }
        }
        None
    }

    fn video_out_pad(&self) -> gst::GhostPad {
        self.video_out_pad.clone()
    }

    fn audio_out_pad(&self) -> gst::GhostPad {
        self.audio_out_pad.clone()
    }

    fn bin(&self) -> gst::Bin {
        self.bin.clone()
    }

    /// Get source pad after overlays shall be inserted.
    fn overlays(&mut self) -> &mut Overlays {
        &mut self.overlays
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
    pub async fn receive_offer(&self, offer: String) -> String {
        trace!("receive_offer()");

        let offer = gst_webrtc::WebRTCSessionDescription::new(
            gst_webrtc::WebRTCSDPType::Offer,
            gst_sdp::SDPMessage::parse_buffer(offer.as_bytes())
                .expect("failed to parse webrtc offer"),
        );

        self.webrtcbin
            .emit_by_name::<()>("set-remote-description", &[&offer, &None::<gst::Promise>]);

        let (send, recv) = oneshot::channel();

        let on_create_answer = {
            let webrtcbin = self.webrtcbin.clone();

            gst::Promise::with_change_func(move |answer| {
                let answer = answer
                    .expect("with_change_func() error")
                    .expect("with_change_func() no result")
                    .get::<gst_webrtc::WebRTCSessionDescription>("answer")
                    .expect("webrtc answer seems to be no answer");

                webrtcbin
                    .emit_by_name::<()>("set-local-description", &[&answer, &None::<gst::Promise>]);

                send.send(answer.sdp().to_string())
                    .expect("failed to send answer SDP to main webrtc main thread");
            })
        };

        // Call create-answer
        self.webrtcbin.emit_by_name::<()>(
            "create-answer",
            &[&None::<gst::Structure>, &on_create_answer],
        );

        recv.await.expect("failed waiting for SDP answer")
    }

    pub async fn receive_candidate(&self, mline: u32, candidate: String) {
        trace!("receive_candidate()");

        self.webrtcbin
            .emit_by_name("add-ice-candidate", &[&mline, &candidate])
    }

    pub async fn receive_end_of_candidates(&self, mline: u32) {
        trace!("receive_end_of_candidates()");

        self.webrtcbin
            .emit_by_name("add-ice-candidate", &[&mline, &None::<String>])
    }
}
