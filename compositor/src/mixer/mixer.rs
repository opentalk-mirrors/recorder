use std::collections::HashMap;

use super::helpers::*;
use super::layout::*;
use super::*;
use gst::prelude::*;
use gstreamer as gst;

/// pipeline elements and pads relating to the video mixer
#[derive(Clone)]
pub struct VideoMixer {
    /// bin the compositor is enveloped in
    pub bin: gst::Bin,
    /// the compositor itself
    pub mixer: gst::Element,
    /// output pad
    pub pad: gst::GhostPad,
    /// input pads
    pub pads: Vec<gst::GhostPad>,
    ///
    pub sources: HashMap<String, gst::GhostPad>,
}

/// pipeline elements and pads relating to the audio mixer
#[derive(Clone)]
pub struct AudioMixer {
    pub bin: gst::Bin,
    pub mixer: gst::Element,
    pub pad: gst::GhostPad,
}

/// Manages interface to mixer pipelines.
///
/// A mixer which puts together meta information (like title, clock or who's speaking) and audio/video
/// from multiple participants (sources).
#[derive(Clone)]
pub struct Mixer {
    pub pipeline: gst::Pipeline,
    /// access the 'who's speaking?' text within the mixer view if provided
    speaking: Option<gst::Element>,
    /// access the title text within the mixer view if provided
    title: Option<gst::Element>,

    pub video: VideoMixer,
    pub audio: AudioMixer,
}

#[allow(dead_code)]
impl Mixer {
    /// Create and start a new mixer and return an communication unit to it.
    /// # Arguments
    /// - `num_viewers`: number of viewers beside a speaker
    /// (so if `num_viewers` is `0` there is only one participant visible)
    /// - `resolution`: target resolution of the output image
    /// - `test_sink`: Use test sinks (video display and audio output on your device)
    pub fn new(resolution: &Size, test_sink: bool) -> Self {
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
        // add composite view to pipeline
        // create and link video mixer
        let video = Self::create_video(&pipeline, resolution);
        // create and link audio mixer
        let audio = Self::create_audio(&pipeline);
        // link srcs to sinks
        video.pad.link(&video_sink).unwrap();
        audio.pad.link(&audio_sink).unwrap();

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
            video,
            audio,
        }
    }
    /// create an video mixer from a given layout
    /// # Arguments
    /// - `pipeline` : the pipeline to add the video mixer into
    /// - `layout` : Layout of speaker and viewers
    /// # Returns
    /// Returns two `GhostPad` instances: 1st for video and 2nd for audio
    #[allow(dead_code)]
    fn create_video(pipeline: &gst::Pipeline, resolution: &Size) -> VideoMixer {
        // prepare a bin with the compositor
        let bin = format!(
            r#"name=compositor-bin
    videotestsrc
        pattern=black
    ! compositor
        name=video-mixer
        background=black
        ignore-inactive-pads=true
    ! clockoverlay
        name=clock
        font-desc="Sans, 14"
        time-format="%x %X %Z"
        xpad=10
        ypad=2
        color=0xffffffff
    ! textoverlay
        name=title
        font-desc="Sans, 16"
        xpad=10
        ypad=2
        color=0xffffffff
    ! textoverlay
        name=speaking
        font-desc="Sans, 16"
        xpad=10
        ypad=2
        color=0xffffffff
    ! video/x-raw,width={width},height={height}
    ! queue
        name=video-mixer-output
                "#,
            width = resolution.width,
            height = resolution.height,
        );

        // parse bin and add it to the pipeline
        info!("parsing video mixer bin:\n{bin}");
        let bin = gst::parse_bin_from_description(&bin, false).unwrap();
        pipeline.add(&bin).unwrap();
        let mixer = bin.by_name("video-mixer").unwrap();
        // link our internal sink to a ghost pad at the bin's outside
        let pad = link_bin_ghost_pad(&bin, "video-mixer-output", "src");
        // return pads of interest
        VideoMixer {
            bin,
            mixer,
            pad,
            pads: Vec::new(),
            sources: HashMap::new(),
        }
    }
    /// create an audio mixer from a given layout
    /// # Arguments
    /// - `pipeline` : the pipeline to add the audio mixer into
    /// - `layout` : Layout of speaker and viewers
    /// # Returns
    /// Returns two `GhostPad` instances: 1st for video and 2nd for audio
    fn create_audio(pipeline: &gst::Pipeline) -> AudioMixer {
        // prepare a bin with the compositor
        let bin = format!(
            r#"name=audio-mixer-bin
    audiotestsrc 
        is_live=true
        volume=0.01
    ! audiomixer
        name=audio-mixer
    ! queue
        name=audio-mixer-output
    "#,
        );

        // parse bin and add it to the pipeline
        info!("parsing audio mixer bin:\n{bin}");
        let bin = gst::parse_bin_from_description(&bin, false).unwrap();
        pipeline.add(&bin).unwrap();

        // get mixer and create output ghost pad
        let mixer = pipeline.by_name("audio-mixer").unwrap();
        let pad = link_bin_ghost_pad(&bin, "audio-mixer-output", "src");
        // link our internal sink to a ghost pad at the bin's outside
        AudioMixer { bin, mixer, pad }
    }
    fn relayout(&self, count: usize, layout: &dyn Layout) {
        self.layout_overlay(
            "title",
            layout.title_position(count),
            layout.title_alignment(),
        );
        self.layout_overlay(
            "clock",
            layout.clock_position(count),
            layout.clock_alignment(),
        );
        self.layout_overlay(
            "speaking",
            layout.speaking_position(count),
            layout.speaking_alignment(count),
        );
        self.video.mixer.foreach_sink_pad(move |_, pad| {
            let n: usize = pad
                .name()
                .split("_")
                .last()
                .unwrap()
                .parse::<usize>()
                .unwrap();
            if n > 0 {
                pad.set_property("xpos", layout.position(n - 1, count).x as i32);
                pad.set_property("ypos", layout.position(n - 1, count).y as i32);
                pad.set_property("width", layout.size(n - 1, count).width as i32);
                pad.set_property("height", layout.size(n - 1, count).height as i32);
            }
            true
        });
    }
    fn layout_overlay(&self, name: &str, position: Position, alignment: Alignment) {
        if let Some(title) = self.pipeline.by_name(name) {
            title.set_property_from_str("halignment", alignment.horizontal);
            title.set_property_from_str("valignment", alignment.vertical);
            title.set_property_from_str("line-alignment", alignment.horizontal);
            title.set_property_from_str("deltax", &position.x.to_string());
            title.set_property_from_str("deltay", &position.y.to_string());
        }
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

    pub fn add_test_source(&mut self, layout: &dyn Layout, name: &str, resolution: &Size) {
        self.pipeline.set_state(gst::State::Paused).unwrap();
        let (_bin, video_source, audio_source) =
            create_test_source(&self.pipeline, name, resolution);
        let audio_pad = gst::GhostPad::with_target(
            None,
            &self.audio.mixer.request_pad_simple("sink_%").unwrap(),
        )
        .unwrap();
        self.audio.bin.add_pad(&audio_pad).unwrap();
        audio_source.link(&audio_pad).unwrap();

        let inner_mixer_pad = self.video.mixer.request_pad_simple("sink_%").unwrap();
        let video_pad = gst::GhostPad::with_target(None, &inner_mixer_pad).unwrap();
        self.video.bin.add_pad(&video_pad).unwrap();
        self.video.pads.push(video_pad);
        self.video.sources.insert(name.to_string(), video_source);
        self.relayout(self.video.sources.len(), layout);
        self.pipeline.set_state(gst::State::Playing).unwrap();
    }

    pub async fn add_stream(&mut self, name: &str, sdp_offer: &str) -> String {
        // create and link speaker's source
        let (webrtcbin, answer) =
            web_rtc_bin::create_web_rtc_bin(&self.pipeline, "video-source-{name}", sdp_offer).await;

        let audio_pad = gst::GhostPad::with_target(
            None,
            &self.audio.mixer.request_pad_simple("sink_%").unwrap(),
        )
        .unwrap();
        self.audio.bin.add_pad(&audio_pad).unwrap();
        webrtcbin.audio_src.link(&audio_pad).unwrap();
        self.video
            .sources
            .insert(name.to_string(), webrtcbin.video_src);

        answer
    }

    pub fn set_viewable(&self, names: &[&str]) {
        self.pipeline.set_state(gst::State::Paused).unwrap();
        for (i, &name) in names.iter().enumerate() {
            self.video
                .sources
                .get(name)
                .unwrap()
                .link(&self.video.pads[i])
                .unwrap();
        }
        self.pipeline.set_state(gst::State::Playing).unwrap();
    }

    /// generate a DOT file describing the current pipeline in a graph (for debugging)
    pub fn generate_dot_file(&self, file_name: &str) {
        gst::debug_bin_to_dot_file(&self.pipeline, gst::DebugGraphDetails::ALL, file_name);
    }
}

pub(crate) fn on_linked(
    orig_src: gst::Element,
    fake_sink: gst::Element,
    ghost_pad: gst::GhostPad,
) -> impl Fn(&[gst::glib::Value]) -> Option<gst::glib::Value> {
    move |_| {
        fake_sink.set_state(gst::State::Ready).unwrap();
        orig_src.unlink(&fake_sink);
        ghost_pad
            .set_target(Some(&orig_src.static_pad("src").unwrap()))
            .unwrap();
        orig_src.set_state(gst::State::Playing).unwrap();
        None
    }
}

pub(crate) fn on_unlinked(
    orig_src: gst::Element,
    fake_sink: gst::Element,
    ghost_pad: gst::GhostPad,
) -> impl Fn(&[gst::glib::Value]) -> Option<gst::glib::Value> {
    move |_| {
        orig_src.set_state(gst::State::Ready).unwrap();
        ghost_pad.set_target(None::<&gst::Pad>).unwrap();
        orig_src.link(&fake_sink).unwrap();
        fake_sink.set_state(gst::State::Playing).unwrap();
        None
    }
}
