use crate::mixer::mixer::*;
use gst::prelude::*;
use gst::Promise;
use gstreamer as gst;
use tokio::sync::oneshot;

pub struct WebRtcSource {
    bin: gst::Bin,
    webrtcbin: gst::Element,

    video_decode: gst::Element,
    video_sink: gst::Element,
    video_fakesink: Option<gst::Element>,
    video_sink_pad: gst::Pad,
    video_ghostpad: gst::Pad,

    audio_decode: gst::Element,
    audio_sink: gst::Element,
    audio_fakesink: Option<gst::Element>,
    audio_sink_pad: gst::Pad,
    audio_ghostpad: gst::Pad,
}

impl Source for WebRtcSource {
    fn new(pipeline: &gst::Pipeline, name: &str, pattern: &str, resolution: &crate::Size) -> Self {
        let bin = gst::parse_bin_from_description(
            "
            webrtcbin
                name=webrtc
                bundle-policy=max-bundle

            webrtc.
            ! rtpvp8depay
            ! vp8dec name=video-decode

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

        let video_fakesink = gst::ElementFactory::make("fakesink", None).unwrap();
        let audio_fakesink = gst::ElementFactory::make("fakesink", None).unwrap();

        let video_fakesink_sink = video_fakesink.static_pad("sink").unwrap();
        let audio_fakesink_sink = audio_fakesink.static_pad("sink").unwrap();

        bin.add_pad(&video_ghostpad).unwrap();
        bin.add_pad(&audio_ghostpad).unwrap();

        pipeline.add(&video_fakesink).unwrap();
        pipeline.add(&audio_fakesink).unwrap();

        video_ghostpad.link(&video_fakesink_sink).unwrap();
        audio_ghostpad.link(&audio_fakesink_sink).unwrap();

        Self {
            bin,
            webrtcbin,
            video_decode,
            video_sink: video_fakesink.clone(),
            video_fakesink: Some(video_fakesink),
            video_sink_pad: video_fakesink_sink,
            video_ghostpad: video_ghostpad.upcast::<gst::Pad>(),
            audio_decode,
            audio_sink: audio_fakesink.clone(),
            audio_fakesink: Some(audio_fakesink),
            audio_sink_pad: audio_fakesink_sink,
            audio_ghostpad: audio_ghostpad.upcast::<gst::Pad>(),
        }
    }

    fn remove(&self, pipeline: &gst::Pipeline) {
        pipeline.remove(&self.bin).unwrap();
    }

    fn video_src_element(&self) -> &gst::Element {
        &self.webrtcbin
    }

    fn video_src_pad(&self) -> &gst::Pad {
        &self.video_ghostpad
    }

    fn video_sink_pad(&self) -> &gst::Pad {
        &self.video_sink_pad
    }

    fn video_fake_sink(&self) -> &Option<gst::Element> {
        &self.video_fakesink
    }

    fn set_video_sink(&mut self, sink_pad: gst::Pad, sink: gst::Element) {
        self.video_sink = sink;
        self.video_sink_pad = sink_pad;
    }

    fn set_video_fake_sink(&mut self, fake_sink: Option<gst::Element>) {
        self.video_fakesink = fake_sink;
    }

    fn audio_src_element(&self) -> &gst::Element {
        &self.webrtcbin
    }

    fn audio_src_pad(&self) -> &gst::Pad {
        &self.audio_ghostpad
    }

    fn audio_sink_pad(&self) -> &gst::Pad {
        &self.audio_sink_pad
    }

    fn audio_fake_sink(&self) -> &Option<gst::Element> {
        &self.audio_fakesink
    }

    fn set_audio_sink(&mut self, sink_pad: gst::Pad, sink: gst::Element) {
        self.audio_sink = sink;
        self.audio_sink_pad = sink_pad;
    }

    fn set_audio_fake_sink(&mut self, fake_sink: Option<gst::Element>) {
        self.audio_fakesink = fake_sink;
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
