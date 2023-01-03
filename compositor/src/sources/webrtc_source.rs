use crate::*;

use gst::prelude::*;
use tokio::sync::oneshot;

/// Source that connects to an WebRTC source and provides the incoming streams as participant's input.
pub struct WebRtcSource {
    /// GStreamer bin surrounding all included elements
    bin: gst::Bin,
    /// WebRTC GStreamer element which manages mostly everything.
    webrtcbin: gst::Element,
    /// GStreamer video ghost pad to connect from the outside of the bin.
    video_ghostpad: gst::Pad,
    /// GStreamer audio ghost pad to connect from the outside of the bin.
    audio_ghostpad: gst::Pad,
    /// Sourec overlays.
    overlays: Overlays,
}

type OnCandidateCallback = Box<dyn Fn(u32, Option<String>) + Send + Sync>;

#[derive(Default)]
pub struct WebRtcSourceParams {
    on_ice_candidate: Option<OnCandidateCallback>,
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
    fn new(pipeline: &gst::Pipeline, _: &Size, params: Self::Parameters) -> Self {
        debug!("create new WebRtcSource");

        let bin = gst::parse_bin_from_description(
            "
            webrtcbin
                name=webrtc
                bundle-policy=max-bundle

            webrtc.
            ! rtpvp8depay
            ! avdec_vp8
            ! videoconvert
                name=overlays
            ! queue
                name=video-output

            webrtc.
            ! rtpopusdepay
            ! opusdec
            ! audioconvert
            ! queue
                name=audio-output
            ",
            false,
        )
        .expect("Failed to parse and load WebRtc pipeline. Is a gst plugin missing?");

        pipeline
            .add(&bin)
            .expect("failed to add WebRtc bin to pipeline");

        let webrtcbin = bin
            .by_name("webrtc")
            .expect("failed to find webrtc in pipeline");

        let video_output = bin
            .by_name("video-output")
            .expect("failed to find webrtc video-output in pipeline");
        let video_output_src = video_output
            .static_pad("src")
            .expect("failed to get static source pad from webrtc video output");

        let audio_output = bin
            .by_name("audio-output")
            .expect("failed to find webrtc audio-output in pipeline");
        let audio_output_src = audio_output
            .static_pad("src")
            .expect("failed to get static source pad from webrtc audio output");

        let video_ghostpad = gst::GhostPad::with_target(Some("video"), &video_output_src)
            .expect("failed to create ghost pad for webrtc video output");
        let audio_ghostpad = gst::GhostPad::with_target(Some("audio"), &audio_output_src)
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

        // get elements from bin
        let overlays = bin
            .by_name("overlays")
            .expect("failed to get overlays from pipeline");

        let overlay_src = overlays
            .static_pad("src")
            .expect("failed to get src pad from overlays");
        let overlay_sink = video_output
            .static_pad("sink")
            .expect("failed to get src pad from video_out ");

        let overlays = Overlays::new(&bin, overlay_src, overlay_sink);

        Self {
            bin,
            webrtcbin,
            video_ghostpad: video_ghostpad.upcast::<gst::Pad>(),
            audio_ghostpad: audio_ghostpad.upcast::<gst::Pad>(),
            overlays,
        }
    }

    fn remove(self, pipeline: &gst::Pipeline) {
        assert!(!self.video_ghostpad.is_linked());
        assert!(!self.audio_ghostpad.is_linked());

        self.bin
            .set_state(gst::State::Null)
            .expect("failed set WebRTC bin to Null");
        pipeline
            .remove(&self.bin)
            .expect("failed to remove webrtc bin from pipeline");
    }

    fn video_src_pad(&self) -> gst::Pad {
        self.video_ghostpad.clone()
    }

    fn audio_src_pad(&self) -> gst::Pad {
        self.audio_ghostpad.clone()
    }

    /// Get source pad after overlays shall be inserted.
    fn overlays(&mut self) -> &mut Overlays {
        &mut self.overlays
    }
}

impl WebRtcSource {
    pub async fn receive_offer(&self, offer: String) -> String {
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
        self.webrtcbin
            .emit_by_name("add-ice-candidate", &[&mline, &candidate])
    }

    pub async fn receive_end_of_candidates(&self, mline: u32) {
        self.webrtcbin
            .emit_by_name("add-ice-candidate", &[&mline, &None::<String>])
    }
}
