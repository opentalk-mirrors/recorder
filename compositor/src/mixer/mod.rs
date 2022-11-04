// sub-modules
mod dash_sink;
mod display_sink;
mod fake_sink;
mod matroska_sink;
mod participant;
mod test_source;
mod webrtc_source;

// forward useful sub-module stuff as public
pub use dash_sink::{DashParameters, DashSink, SegmentType};
pub use display_sink::DisplaySink;
pub use fake_sink::FakeSink;
pub use matroska_sink::{MatroskaParameters, MatroskaSink};
pub use participant::Participant;
pub use test_source::{TestSource, TestSourceParameters};
pub use webrtc_source::WebRtcSource;

// what else we need from this lib
use crate::{error::Error, mixer::participant::VideoLinkStatus, Alignment, Layout, Position, Size};

// what we need from external libraries
use core::mem::replace;
use gst::prelude::*;
use gstreamer as gst;
use std::collections::HashMap;

/// Trait of a participant's audio/video source.
pub trait Source {
    /// Generic parameter type to overwrite by trait implementers.
    type Parameters;
    /// Create an add a new source to a pipeline.
    /// Creates a bunch of elements based on given parameters and adds them to the pipeline.
    fn new(pipeline: &gst::Pipeline, id: String, params: Self::Parameters) -> Self;
    /// Remove existing source from pipeline.
    /// Decouples and removes all elements from the pipeline which are created within this source.
    fn remove(self, pipeline: &gst::Pipeline);
    /// Get source pad of the video source.
    fn video_src_pad(&self) -> gst::Pad;
    /// Get source pad of the audio source.
    fn audio_src_pad(&self) -> gst::Pad;
}

/// Trait of an output sink.
pub trait Sink {
    /// Generic parameter type to overwrite by trait implementers.
    type Parameters;
    /// Create an add a sink to the pipeline.
    /// Creates a bunch of elements based on given parameters and adds them to the pipeline.
    fn new(pipeline: &gst::Pipeline, params: Self::Parameters) -> Self;
    /// Get sink pad of the video sink.
    fn video_sink_pad(&self) -> gst::Pad;
    /// Get sink pad of the audio sink.
    fn audio_sink_pad(&self) -> gst::Pad;
    /// called from `Mixer::play()`
    fn on_play(&self) {}
    /// called from `Mixer::play()`
    fn on_pause(&self) {}
}

/// Mixer managing the GStreamer pipeline using the given layout and source type
/// # Types
/// - `L`: Layout to use to compose output picture.
/// - `SRC`: Source type to use when adding participants.
/// - `SINK`: Sink type to use for output.
pub struct Mixer<L, SRC, SINK>
where
    L: Layout,
    SRC: Source,
    SINK: Sink,
{
    /// GStreamer element which composes the output video out of the source videos.
    pub compositor: gst::Element,
    /// GStreamer element which composes the output audio out of the source audios.
    pub audio_mixer: gst::Element,
    /// Maximum number of visible participants.
    pub max_visibles: usize,
    /// Number of currently visible participants.
    pub visibles: usize,
    /// GStreamer element for rendering a clock into the output picture if whished.
    clock: Option<gst::Element>,
    /// GStreamer element for rendering a title into the output picture if whished.
    title: Option<gst::Element>,
    /// GStreamer element for rendering a "who' speaking" display into the output picture if whished.
    speaking: Option<gst::Element>,
    /// The mixer GStreamer pipeline.
    pipeline: gst::Pipeline,
    /// Layout of the output picture.
    layout: L,
    /// Current participants.
    pub participants: HashMap<String, Participant<SRC>>,
    /// Holds the output sink.
    pub output: SINK,
}

impl<L, SRC, SINK> Mixer<L, SRC, SINK>
where
    L: Layout,
    SRC: Source,
    SINK: Sink,
{
    /// Create a new mixer and setup the initial GStreamer pipeline with the given type of sink.
    /// # Arguments
    /// - `resolution`: Output video resolution.
    /// - `max_visibles`: Maximum number of visible participants.
    /// - `visibles`: Number of currently visible participants.
    pub fn new(
        resolution: Size,
        max_visibles: usize,
        sink_params: SINK::Parameters,
    ) -> Result<Self, Error> {
        // get width/height
        let width = resolution.width;
        let height = resolution.height;
        // create new layout for the given resolution
        let layout = L::new(&resolution);
        // create new GStreamer pipeline
        let pipeline = gst::Pipeline::new(None);

        // create output link
        let output = SINK::new(&pipeline, sink_params);

        // create video test src to get a picture when no participant is connected
        let video_background_src =
            gst::ElementFactory::make_with_name("videotestsrc", Some("video-background")).unwrap();
        video_background_src.set_property_from_str("pattern", "black");
        video_background_src.set_property_from_str("is-live", "true");

        // create video caps setter
        let video_caps =
            gst::ElementFactory::make_with_name("capssetter", Some("video-caps")).unwrap();
        video_caps.set_property_from_str(
            "caps",
            &format!("video/x-raw,format=RGB,width={width},height={height}",),
        );

        // create video compositor
        let compositor =
            gst::ElementFactory::make_with_name("compositor", Some("video-compositor")).unwrap();
        compositor.set_property_from_str("ignore-inactive-pads", "true");
        for _ in 0..max_visibles + 1 {
            compositor.request_pad_simple("sink_%u").unwrap();
        }

        // create video clock overlay
        let clock_overlay =
            gst::ElementFactory::make_with_name("clockoverlay", Some("video-clock-overlay"))
                .unwrap();
        clock_overlay.set_property_from_str("font-desc", "Sans, 14");
        clock_overlay.set_property_from_str("time-format", "%x %X %Z");
        clock_overlay.set_property_from_str("xpad", "10");
        clock_overlay.set_property_from_str("ypad", "2");
        clock_overlay.set_property_from_str("color", "0xffffffff");

        // create video title text overlay
        let title_overlay =
            gst::ElementFactory::make_with_name("textoverlay", Some("video-title-overlay"))
                .unwrap();
        title_overlay.set_property_from_str("font-desc", "Sans, 16");
        title_overlay.set_property_from_str("xpad", "10");
        title_overlay.set_property_from_str("ypad", "2");
        title_overlay.set_property_from_str("color", "0xffffffff");

        // create video speaking text overlay
        let speaking_overlay =
            gst::ElementFactory::make_with_name("textoverlay", Some("video-speaking-overlay"))
                .unwrap();
        speaking_overlay.set_property_from_str("font-desc", "Sans, 16");
        speaking_overlay.set_property_from_str("xpad", "10");
        speaking_overlay.set_property_from_str("ypad", "2");
        speaking_overlay.set_property_from_str("color", "0xffffffff");

        let video_output_pad = speaking_overlay.static_pad("src").unwrap();

        // add video elements to pipeline
        pipeline.add(&video_background_src).unwrap();
        pipeline.add(&video_caps).unwrap();
        pipeline.add(&compositor).unwrap();
        pipeline.add(&clock_overlay).unwrap();
        pipeline.add(&title_overlay).unwrap();
        pipeline.add(&speaking_overlay).unwrap();

        // link video elements
        video_background_src.link(&video_caps).unwrap();
        video_caps.link(&compositor).unwrap();
        compositor.link(&clock_overlay).unwrap();
        clock_overlay.link(&title_overlay).unwrap();
        title_overlay.link(&speaking_overlay).unwrap();
        // link to output sink
        video_output_pad.link(&output.video_sink_pad()).unwrap();

        // create test src to get a picture when no participant is connected
        let audio_background_src =
            gst::ElementFactory::make_with_name("audiotestsrc", Some("audio-background")).unwrap();
        audio_background_src.set_property_from_str("is-live", "true");
        audio_background_src.set_property_from_str("volume", "0.0");

        // create audio caps setter
        let audio_caps =
            gst::ElementFactory::make_with_name("capssetter", Some("audio-caps")).unwrap();
        audio_caps.set_property_from_str(
            "caps",
            "audio/x-raw,format=S16LE,channels=2,layout=interleaved,rate=48000",
        );

        // create audio mixer
        let audio_mixer =
            gst::ElementFactory::make_with_name("audiomixer", Some("audio-mixer")).unwrap();
        let audio_output_pad = audio_mixer.static_pad("src").unwrap();

        // link audio elements
        pipeline.add(&audio_background_src).unwrap();
        pipeline.add(&audio_caps).unwrap();
        pipeline.add(&audio_mixer).unwrap();

        // link audio elements
        audio_background_src.link(&audio_caps).unwrap();
        audio_caps.link(&audio_mixer).unwrap();
        // link to output sink
        audio_output_pad.link(&output.audio_sink_pad()).unwrap();

        Ok(Mixer {
            // remember all those elements and pads
            compositor,
            audio_mixer,
            max_visibles,
            visibles: 0,
            clock: Some(clock_overlay),
            title: Some(title_overlay),
            speaking: Some(speaking_overlay),
            layout,
            pipeline,
            participants: HashMap::new(),
            output,
        })
    }
    /// Add a new participant to the mixer.
    /// # Arguments
    /// - `id`: Unique identifier of the participant.
    /// - `params`: Source specific parameters.
    pub fn add_participant(
        &mut self,
        id: String,
        display_name: String,
        params: SRC::Parameters,
    ) -> Result<(), Error> {
        // check preconditions
        if self.pipeline.current_state() == gst::State::Playing {
            return Err(Error::PlayingPipelineForbidden);
        }
        if self.participants.contains_key(&id) {
            return Err(Error::IdDoublet(id));
        }

        // add new participant
        let participant = Participant::new(&self.pipeline, id.clone(), display_name, params);
        self.participants.insert(id.to_string(), participant);

        // link new participant
        self.link_audio(&id)?;
        self.link_video_to_fakesink(&id)?;

        // re-layout
        self.layout()?;

        Ok(())
    }

    /// remove an once added participant from the mixer.
    /// # Arguments
    /// - `id`: Unique identifier of the participant.
    pub fn remove_participant(&mut self, id: &str) -> Result<(), Error> {
        // check preconditions
        if self.pipeline.current_state() == gst::State::Playing {
            return Err(Error::PlayingPipelineForbidden);
        }

        // unlink participant from rest of the pipeline
        self.unlink_audio(id)?;
        self.unlink_video(id)?;

        // remove participant from stored participants
        let participant = self
            .participants
            .remove(id)
            .ok_or_else(|| Error::ParticipantNotFound(id.to_owned()))?;

        // remove participant's source from pipeline
        participant.source.remove(&self.pipeline);

        // re-layout
        self.layout()?;

        Ok(())
    }

    /// Select the participants which are visible.
    /// All previously visible participants get invisible if they are not in the list.
    /// # Arguments
    /// - `ids`: List of identifiers of participants which shall get visible
    pub fn set_visibles(&mut self, ids: &[String]) -> Result<(), Error> {
        // check preconditions
        if self.pipeline.current_state() == gst::State::Playing {
            return Err(Error::PlayingPipelineForbidden);
        }

        // check if given list exceeds maximum length
        if ids.len() > self.max_visibles {
            return Err(Error::TooManyVisibles);
        }

        // Unlink all participants
        for id in self.participants.keys().cloned().collect::<Vec<_>>() {
            self.link_video_to_fakesink(&id)?;
        }
        self.visibles = 0;

        // Link all given participants
        for (n, id) in ids.iter().enumerate() {
            self.link_video_to_compositor(id, n)?;
            self.visibles += 1;
        }

        // re-layout
        self.layout()?;

        Ok(())
    }

    /// set the title text within the mixer view if provided
    pub fn set_title(&self, text: &str) {
        if let Some(title) = &self.title {
            title.set_property("text", text);
        }
    }

    /// set the 'who's speaking?' text within the mixer view if provided
    pub fn set_speaking(&self, text: &str) {
        if let Some(speaking) = &self.speaking {
            speaking.set_property("text", text);
        }
    }

    /// start playing of pipeline
    pub fn play(&self) {
        self.pipeline.set_state(gst::State::Playing).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(100));
        self.output.on_play();
    }

    /// pause playing of pipeline
    pub fn pause(&self) {
        self.pipeline.set_state(gst::State::Paused).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(100));
        self.output.on_pause();
    }

    /// generate DOT file of the current pipeline
    pub fn generate_dot_file(
        &self,
        filename_without_extension: &str,
        details: gst::DebugGraphDetails,
    ) {
        if let Ok(path) = std::env::var("GST_DEBUG_DUMP_DOT_DIR") {
            info!(
                "writing DOT file `{}/{filename_without_extension}.dot`...",
                path
            );
            gst::debug_bin_to_dot_file(&self.pipeline, details, filename_without_extension);
        } else {
            error!("can not write DOT file. You need to set GST_DEBUG_DUMP_DOT_DIR in environment to a absolute path");
        }
    }
}

impl<L, SRC, SINK> Mixer<L, SRC, SINK>
where
    L: Layout,
    SRC: Source,
    SINK: Sink,
{
    /// wait until mixer generates error or ends
    fn run(&self) {
        // wait until error or EOS
        let bus = self.pipeline.bus().unwrap();
        for msg in bus.iter_timed(gst::ClockTime::NONE) {
            use gst::MessageView;

            match msg.view() {
                MessageView::Error(err) => {
                    error!(
                        "Error received from element {:?}: {}",
                        err.src().map(|s| s.path_string()),
                        err.error()
                    );
                    debug!("Debugging information: {:?}", err.debug());
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
        trace!("Mixer exited successfully")
    }

    /// Re-layout the current compositor scene.
    fn layout(&self) -> Result<(), Error> {
        // check preconditions
        if self.pipeline.current_state() == gst::State::Playing {
            return Err(Error::PlayingPipelineForbidden);
        }

        // layout overlays
        self.layout_overlay(
            &self.title,
            self.layout.title_position(self.visibles),
            self.layout.title_alignment(),
        );
        self.layout_overlay(
            &self.clock,
            self.layout.clock_position(self.visibles),
            self.layout.clock_alignment(),
        );
        self.layout_overlay(
            &self.speaking,
            self.layout.speaking_position(self.visibles),
            self.layout.speaking_alignment(self.visibles),
        );

        // configure compositor sink pads (which might be connected to the participants' sources)
        for (n, pad) in self.compositor.sink_pads()[1..].iter().enumerate() {
            let (pos, size, alpha) = if n < self.visibles {
                (
                    self.layout.position(n, self.visibles),
                    self.layout.size(n, self.visibles),
                    1.0,
                )
            } else {
                (
                    Position { x: 0, y: 0 },
                    Size {
                        width: 0,
                        height: 0,
                    },
                    0.0,
                )
            };
            trace!(
                "{name}: xpos={xpos}, ypos={ypos}, width={width}, height={height}",
                xpos = pos.x as i32,
                ypos = pos.y as i32,
                width = size.width as i32,
                height = size.height as i32,
                name = pad.name()
            );
            pad.set_property("xpos", pos.x as i32);
            pad.set_property("ypos", pos.y as i32);
            pad.set_property("width", size.width as i32);
            pad.set_property("height", size.height as i32);
            pad.set_property("alpha", alpha);
        }

        Ok(())
    }

    /// Layout an GstBaseTextOverlay derivate.
    fn layout_overlay(
        &self,
        element: &Option<gst::Element>,
        position: Position,
        alignment: Alignment,
    ) {
        if let Some(element) = element {
            element.set_property_from_str("halignment", alignment.horizontal);
            element.set_property_from_str("valignment", alignment.vertical);
            element.set_property_from_str("line-alignment", alignment.horizontal);
            element.set_property_from_str("deltax", &position.x.to_string());
            element.set_property_from_str("deltay", &position.y.to_string());
        }
    }

    /// Link participant's audio source to audio mixer.
    fn link_audio(&mut self, id: &str) -> Result<(), Error> {
        trace!("linking audio of {:?}...", id);

        let participant = self
            .participants
            .get_mut(id)
            .ok_or_else(|| Error::ParticipantNotFound(id.to_owned()))?;

        let mixer_pad = self.audio_mixer.request_pad_simple("sink_%").unwrap();

        participant.source.audio_src_pad().link(&mixer_pad).unwrap();

        participant.audio_mixer_pad = Some(mixer_pad);

        Ok(())
    }

    /// Unlink participant's audio from the audiomixer.
    fn unlink_audio(&mut self, id: &str) -> Result<(), Error> {
        trace!("unlinking audio of {id:?}...");

        let participant = self
            .participants
            .get_mut(id)
            .ok_or_else(|| Error::ParticipantNotFound(id.to_owned()))?;

        if let Some(pad) = participant.audio_mixer_pad.take() {
            participant.source.audio_src_pad().unlink(&pad).unwrap();
        }

        Ok(())
    }

    /// Link participant's video source to fake sink (while it's invisible).
    fn link_video_to_fakesink(&mut self, id: &str) -> Result<(), Error> {
        trace!("linking video of {id:?} to fakesink...");

        let participant = self
            .participants
            .get_mut(id)
            .ok_or_else(|| Error::ParticipantNotFound(id.to_owned()))?;

        match &participant.video_link_status {
            VideoLinkStatus::None => {}
            VideoLinkStatus::Fakesink(_) => return Ok(()),
            VideoLinkStatus::Compositor(_, pad) => {
                participant.source.video_src_pad().unlink(pad).unwrap()
            }
        }

        let fakesink = gst::ElementFactory::make_with_name("fakesink", None).unwrap();
        self.pipeline.add(&fakesink).unwrap();
        participant
            .source
            .video_src_pad()
            .link(&fakesink.static_pad("sink").unwrap())
            .unwrap();

        participant.video_link_status = VideoLinkStatus::Fakesink(fakesink);

        Ok(())
    }

    /// Link participant's source to video compositor.
    fn link_video_to_compositor(&mut self, id: &str, n: usize) -> Result<(), Error> {
        trace!("linking video of {id:?} to compositor@{n}...");

        let participant = self
            .participants
            .get_mut(id)
            .ok_or_else(|| Error::ParticipantNotFound(id.to_owned()))?;

        match &participant.video_link_status {
            VideoLinkStatus::None => {}
            VideoLinkStatus::Fakesink(fakesink) => {
                participant
                    .source
                    .video_src_pad()
                    .unlink(&fakesink.static_pad("sink").unwrap())
                    .unwrap();
                fakesink.set_state(gst::State::Null).unwrap();
                self.pipeline.remove(fakesink).unwrap();
            }
            VideoLinkStatus::Compositor(curr_n, pad) => {
                if *curr_n != n {
                    participant.source.video_src_pad().unlink(pad).unwrap();
                }
            }
        }

        let compositor_sink_pads = self.compositor.sink_pads();

        participant
            .source
            .video_src_pad()
            .link(&compositor_sink_pads[n + 1])
            .unwrap();

        participant.video_link_status =
            VideoLinkStatus::Compositor(n, compositor_sink_pads[n + 1].clone());

        trace!("linked video of {id:?} to compositor@{n}...");

        Ok(())
    }

    // Unlink participant's video source from compositor.
    fn unlink_video(&mut self, id: &str) -> Result<(), Error> {
        trace!("unlinking video of {id:?}...");

        let participant = self
            .participants
            .get_mut(id)
            .ok_or_else(|| Error::ParticipantNotFound(id.to_owned()))?;

        match replace(&mut participant.video_link_status, VideoLinkStatus::None) {
            VideoLinkStatus::None => {}
            VideoLinkStatus::Fakesink(fakesink) => {
                participant
                    .source
                    .video_src_pad()
                    .unlink(&fakesink.static_pad("sink").unwrap())
                    .unwrap();
                fakesink.set_state(gst::State::Null).unwrap();
                self.pipeline.remove(&fakesink).unwrap();
            }
            VideoLinkStatus::Compositor(_, pad) => {
                participant.source.video_src_pad().unlink(&pad).unwrap();
            }
        }

        trace!("unlinked video of {id:?}...");

        Ok(())
    }
}

impl<L, SRC, SINK> Drop for Mixer<L, SRC, SINK>
where
    L: Layout,
    SRC: Source,
    SINK: Sink,
{
    /// halt pipeline (can not be played again)
    fn drop(&mut self) {
        trace!("exiting mixer");
        // send EOS into pipeline to flush output
        self.pipeline.send_event(gst::event::Eos::new());
        // wait for EOS being processed
        self.run();
    }
}
