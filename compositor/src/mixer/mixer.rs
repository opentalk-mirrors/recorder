use std::collections::HashMap;

use super::*;
use gst::prelude::*;
use gstreamer as gst;

/// Manages interface to mixer pipelines.
///
/// A mixer which puts together meta information (like title, clock or who's speaking) and audio/video
/// from multiple participants (sources).
#[derive(Clone)]
pub struct Mixer {
    pub resolution: Size,
    pub pipeline: gst::Pipeline,
    /// access the 'who's speaking?' text within the mixer view if provided
    speaking: Option<gst::Element>,
    /// access the title text within the mixer view if provided
    title: Option<gst::Element>,
    pub video_pads: Vec<gst::GhostPad>,
    pub video_sources: HashMap<String, gst::GhostPad>,
    pub audio_mixer_bin: gst::Bin,
    pub audio_mixer: gst::Element,
}

#[allow(dead_code)]
impl Mixer {
    /// Create and start a new mixer and return an communication unit to it.
    /// # Arguments
    /// - `num_viewers`: number of viewers beside a speaker
    /// (so if `num_viewers` is `0` there is only one participant visible)
    /// - `resolution`: target resolution of the output image
    /// - `test_src`: Use test sources (generate test content instead of using webrtc)
    /// - `test_sink`: Use test sinks (video display and audio output on your device)
    pub fn new(num_viewers: usize, resolution: Size, test_src: bool, test_sink: bool) -> Self {
        // print pipeline in verbose mode
        info!("parsing pipeline...");

        // create an empty pipeline
        let pipeline = gst::Pipeline::new(Some("Mixer"));

        // add dash sink to pipeline
        let (video_sink, audio_sink) = if test_sink {
            create_display_sink(&pipeline)
        } else {
            create_dash_sink(&pipeline)
        };
        // create layout
        let layout = Layout::new_speaker_vertical(&resolution, num_viewers);
        // add composite view to pipeline
        let (video_src, video_pads, audio_mixer_bin, audio_mixer, audio_src) =
            Mixer::new_speaker(&pipeline, &layout);
        // link srcs to sinks
        video_src.link(&video_sink).unwrap();
        audio_src.link(&audio_sink).unwrap();

        // start playing
        info!("starting pipeline...");
        pipeline
            .set_state(gst::State::Playing)
            .expect("Unable to set the pipeline to the `Playing` state");
        info!("pipeline running (press Ctrl+C to stop)...");

        Self {
            resolution,
            // get elements of interest from pipeline
            title: pipeline.by_name("title"),
            speaking: pipeline.by_name("speaking"),
            pipeline,
            video_pads,
            video_sources: HashMap::new(),
            audio_mixer_bin,
            audio_mixer,
        }
    }

    /// wait until mixer generates error or ends
    pub fn run(&self) {
        // wait until error or EOS
        let bus = self.pipeline.bus().unwrap();
        for msg in bus.iter_timed(gst::ClockTime::NONE) {
            use gst::MessageView;

            match msg.view() {
                MessageView::Error(err) => {
                    eprintln!(
                        "Error received from element {:?}: {}",
                        err.src().map(|s| s.path_string()),
                        err.error()
                    );
                    eprintln!("Debugging information: {:?}", err.debug());
                    break;
                }
                MessageView::Eos(..) => break,
                _ => (),
            }
        }

        // stop pipeline
        self.pipeline
            .set_state(gst::State::Null)
            .expect("Unable to set the pipeline to the `Null` state");
    }

    /// set the 'who's speaking?' text within the mixer view if provided
    pub fn set_title(&self, text: &str) {
        if let Some(title) = &self.title {
            title.set_property("text", text);
        }
    }

    /// set the title text within the mixer view if provided
    pub fn set_speaking(&self, text: &str) {
        if let Some(speaking) = &self.speaking {
            speaking.set_property("text", text);
        }
    }

    pub fn add_test_source(&mut self, name: &str) {
        self.pipeline.set_state(gst::State::Paused).unwrap();
        let (bin, video_source, audio_source) = create_test_source(
            &self.pipeline,
            name,
            &Size {
                width: 1920,
                height: 1080,
            },
        );
        let audio_pad = gst::GhostPad::with_target(
            None,
            &self.audio_mixer.request_pad_simple("sink_%").unwrap(),
        )
        .unwrap();
        self.audio_mixer_bin.add_pad(&audio_pad).unwrap();
        audio_source.link(&audio_pad).unwrap();
        self.video_sources.insert(name.to_string(), video_source);
        self.pipeline.set_state(gst::State::Playing).unwrap();
    }

    pub async fn add_stream(&mut self, name: &str, sdp_offer: &str) -> String {
        // create and link speaker's source
        let (webrtcbin, answer) =
            web_rtc_bin::create_web_rtc_bin(&self.pipeline, "video-source-{name}", sdp_offer).await;

        let audio_pad = gst::GhostPad::with_target(
            None,
            &self.audio_mixer.request_pad_simple("sink_%").unwrap(),
        )
        .unwrap();
        self.audio_mixer_bin.add_pad(&audio_pad).unwrap();
        webrtcbin.audio_src.link(&audio_pad).unwrap();
        self.video_sources
            .insert(name.to_string(), webrtcbin.video_src);

        answer
    }

    pub fn set_viewable(&self, names: &[&str]) {
        self.pipeline.set_state(gst::State::Paused).unwrap();
        for (i, &name) in names.iter().enumerate() {
            self.video_sources
                .get(name)
                .unwrap()
                .link(&self.video_pads[i])
                .unwrap();
        }
        self.pipeline.set_state(gst::State::Playing).unwrap();
    }

    /// generate a DOT file describing the current pipeline in a graph (for debugging)
    pub fn generate_dot_file(&self, file_name: &str) {
        gst::debug_bin_to_dot_file(&self.pipeline, gst::DebugGraphDetails::STATES, file_name);
    }
}
