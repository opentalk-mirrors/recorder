use gst::prelude::*;
use gstreamer as gst;
use tokio::sync::oneshot;

use crate::Source;

pub struct WebRtcSource {
    bin: gst::Bin,
    webrtcbin: gst::Element,

    video_ghostpad: gst::Pad,
    audio_ghostpad: gst::Pad,
}

impl Source for WebRtcSource {
    type Parameters = ();

    fn new(pipeline: &gst::Pipeline, _: ()) -> Self {
        let bin = gst::parse_bin_from_description(
            "
            webrtcbin
                name=webrtc
                bundle-policy=max-bundle

            webrtc.
            ! rtpvp8depay
            ! avdec_vp8 name=video-decode

            webrtc.
            ! rtpopusdepay
            ! opusdec name=audio-decode
        ",
            false,
        )
        .unwrap();

        pipeline.add(&bin).unwrap();

        let webrtcbin = bin.by_name("webrtc").unwrap();

        let video_decode = bin.by_name("video-decode").unwrap();
        let video_decode_src = video_decode.static_pad("src").unwrap();

        let audio_decode = bin.by_name("audio-decode").unwrap();
        let audio_decode_src = audio_decode.static_pad("src").unwrap();

        let video_ghostpad = gst::GhostPad::with_target(Some("video"), &video_decode_src).unwrap();
        let audio_ghostpad = gst::GhostPad::with_target(Some("audio"), &audio_decode_src).unwrap();

        bin.add_pad(&video_ghostpad).unwrap();
        bin.add_pad(&audio_ghostpad).unwrap();

        Self {
            bin,
            webrtcbin,
            video_ghostpad: video_ghostpad.upcast::<gst::Pad>(),
            audio_ghostpad: audio_ghostpad.upcast::<gst::Pad>(),
        }
    }

    fn remove(self, pipeline: &gst::Pipeline) {
        pipeline.remove(&self.bin).unwrap();
    }

    fn video_src_pad(&self) -> gst::Pad {
        self.video_ghostpad.clone()
    }

    fn audio_src_pad(&self) -> gst::Pad {
        self.audio_ghostpad.clone()
    }
}

impl WebRtcSource {
    pub async fn receive_offer(&self, offer: String) -> String {
        let offer = gst_webrtc::WebRTCSessionDescription::new(
            gst_webrtc::WebRTCSDPType::Offer,
            gst_sdp::SDPMessage::parse_buffer(offer.as_bytes()).unwrap(),
        );

        self.webrtcbin
            .emit_by_name::<()>("set-remote-description", &[&offer, &None::<gst::Promise>]);

        let (send, recv) = oneshot::channel();

        let on_create_answer = {
            let webrtcbin = self.webrtcbin.clone();

            gst::Promise::with_change_func(move |answer| {
                let answer = answer
                    .unwrap()
                    .unwrap()
                    .get::<gst_webrtc::WebRTCSessionDescription>("answer")
                    .unwrap();

                webrtcbin
                    .emit_by_name::<()>("set-local-description", &[&answer, &None::<gst::Promise>]);

                send.send(answer.sdp().to_string()).unwrap();
            })
        };

        // Call create-answer
        self.webrtcbin.emit_by_name::<()>(
            "create-answer",
            &[&None::<gst::Structure>, &on_create_answer],
        );

        recv.await.unwrap()
    }
}
