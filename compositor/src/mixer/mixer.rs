use super::*;
use gst::prelude::*;
use gstreamer as gst;

/// Manages interface to mixer pipelines.
///
/// A mixer which puts together meta information (like title, clock or who's speaking) and audio/video
/// from multiple participants (sources).
#[derive(Clone)]
pub struct Mixer {
    pipeline: gst::Pipeline,
    /// access the 'who's speaking?' text within the mixer view if provided
    speaking: Option<gst::Element>,
    /// access the title text within the mixer view if provided
    title: Option<gst::Element>,
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
        let (video_src, audio_src) = Mixer::new_speaker(
            &pipeline,
            &layout,
            if test_src {
                create_test_source
            } else {
                create_web_rtc_bin
            },
        );
        // link sources to sinks
        video_src.link(&video_sink).unwrap();
        audio_src.link(&audio_sink).unwrap();

        // start playing
        info!("starting pipeline...");
        pipeline
            .set_state(gst::State::Playing)
            .expect("Unable to set the pipeline to the `Playing` state");
        info!("pipeline running (press Ctrl+C to stop)...");

        Self {
            // get elements of interest from pipeline
            title: pipeline.by_name("title"),
            speaking: pipeline.by_name("speaking"),
            pipeline,
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

    /// generate a DOT file describing the current pipeline in a graph (for debugging)
    pub fn generate_dot_file(&self, file_name: &str) {
        gst::debug_bin_to_dot_file(&self.pipeline, gst::DebugGraphDetails::STATES, file_name);
    }
}
