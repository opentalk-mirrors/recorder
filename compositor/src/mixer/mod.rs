use crate::{error::Error, mixer::participant::VideoLinkStatus, Alignment, Layout, Position, Size};
use core::mem::replace;
use gst::prelude::*;
use gstreamer as gst;
use std::collections::HashMap;

mod dash_sink;
mod display_sink;
mod participant;
mod test_source;
mod webrtc_source;

pub use dash_sink::DashSink;
pub use display_sink::DisplaySink;
pub use participant::Participant;
pub use test_source::TestSource;
pub use webrtc_source::WebRtcSource;

pub trait Source {
    type Parameters;

    fn new(pipeline: &gst::Pipeline, params: Self::Parameters) -> Self;

    fn remove(self, pipeline: &gst::Pipeline);

    fn video_src_pad(&self) -> gst::Pad;
    fn audio_src_pad(&self) -> gst::Pad;
}

pub trait Sink {
    type Parameters;

    fn new(pipeline: &gst::Pipeline, params: Self::Parameters) -> Self;
    fn video_sink_pad(&self) -> gst::Pad;
    fn audio_sink_pad(&self) -> gst::Pad;
}

pub struct Mixer<L, SRC>
where
    L: Layout,
    SRC: Source,
{
    pub compositor: gst::Element,
    pub audio_mixer: gst::Element,
    pub max_participants: usize,
    pub max_visibles: usize,
    pub visibles: usize,
    clock: Option<gst::Element>,
    title: Option<gst::Element>,
    speaking: Option<gst::Element>,
    pipeline: gst::Pipeline,
    layout: L,
    pub participants: HashMap<String, Participant<SRC>>,
}

impl<L, S> Mixer<L, S>
where
    L: Layout,
    S: Source,
{
    pub fn new<SINK>(
        resolution: Size,
        max_participants: usize,
        max_visibles: usize,
    ) -> Result<Self, Error>
    where
        SINK: Sink<Parameters = ()>,
    {
        if max_participants < max_visibles {
            return Err(Error::MoreMaxVisiblesThanMaxParticipants);
        }
        let width = resolution.width;
        let height = resolution.height;
        let layout = L::new(&resolution);
        let pipeline = gst::Pipeline::new(None);

        // create output link
        let output = SINK::new(&pipeline, ());

        // create video test src to get a picture when no participant is connected
        let video_background_src =
            gst::ElementFactory::make("videotestsrc", Some("video-background")).unwrap();
        video_background_src.set_property_from_str("pattern", "black");
        video_background_src.set_property_from_str("is-live", "true");

        // create video caps setter
        let video_caps = gst::ElementFactory::make("capssetter", Some("video-caps")).unwrap();
        video_caps.set_property_from_str(
            "caps",
            &format!("video/x-raw,format=RGB,width={width},height={height}",),
        );

        // create video compositor
        let compositor = gst::ElementFactory::make("compositor", Some("video-compositor")).unwrap();
        compositor.set_property_from_str("ignore-inactive-pads", "true");
        for _ in 0..max_visibles + 1 {
            compositor.request_pad_simple("sink_%u").unwrap();
        }

        // create video clock overlay
        let clock_overlay =
            gst::ElementFactory::make("clockoverlay", Some("video-clock-overlay")).unwrap();
        clock_overlay.set_property_from_str("font-desc", "Sans, 14");
        clock_overlay.set_property_from_str("time-format", "%x %X %Z");
        clock_overlay.set_property_from_str("xpad", "10");
        clock_overlay.set_property_from_str("ypad", "2");
        clock_overlay.set_property_from_str("color", "0xffffffff");

        // create video title text overlay
        let title_overlay =
            gst::ElementFactory::make("textoverlay", Some("video-title-overlay")).unwrap();
        title_overlay.set_property_from_str("font-desc", "Sans, 16");
        title_overlay.set_property_from_str("xpad", "10");
        title_overlay.set_property_from_str("ypad", "2");
        title_overlay.set_property_from_str("color", "0xffffffff");

        // create video speaking text overlay
        let speaking_overlay =
            gst::ElementFactory::make("textoverlay", Some("video-speaking-overlay")).unwrap();
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
            gst::ElementFactory::make("audiotestsrc", Some("audio-background")).unwrap();
        audio_background_src.set_property_from_str("is-live", "true");
        audio_background_src.set_property_from_str("volume", "0.0");

        // create audio caps setter
        let audio_caps = gst::ElementFactory::make("capssetter", Some("audio-caps")).unwrap();
        audio_caps.set_property_from_str(
            "caps",
            "audio/x-raw,format=S16LE,channels=2,layout=interleaved,rate=48000",
        );

        // create audio mixer
        let audio_mixer = gst::ElementFactory::make("audiomixer", Some("audio-mixer")).unwrap();
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
            max_participants,
            max_visibles,
            visibles: 0,
            clock: Some(clock_overlay),
            title: Some(title_overlay),
            speaking: Some(speaking_overlay),
            layout,
            pipeline,
            participants: HashMap::new(),
        })
    }

    pub fn add_participant(&mut self, name: String, params: S::Parameters) -> Result<(), Error> {
        if self.pipeline.current_state() == gst::State::Playing {
            return Err(Error::PlayingPipelineForbidden);
        }

        if self.participants.len() >= self.max_participants {
            return Err(Error::TooManyParticipants);
        }

        let participant = Participant::new(&self.pipeline, name.clone(), params);
        self.participants.insert(name.to_string(), participant);

        self.link_audio(&name)?;
        self.link_video_to_fakesink(&name)?;

        Ok(())
    }

    pub fn remove_participant(&mut self, name: &str) -> Result<(), Error> {
        if self.pipeline.current_state() == gst::State::Playing {
            return Err(Error::PlayingPipelineForbidden);
        }

        trace!("remove participant {name:?}");

        self.unlink_audio(name)?;
        self.unlink_video(name)?;

        let participant = self
            .participants
            .remove(name)
            .ok_or_else(|| Error::ParticipantNotFound(name.to_owned()))?;

        participant.source.remove(&self.pipeline);

        self.layout()?;

        Ok(())
    }

    pub fn set_visibles(&mut self, names: &[String]) -> Result<(), Error> {
        if self.pipeline.current_state() == gst::State::Playing {
            return Err(Error::PlayingPipelineForbidden);
        }

        if names.len() > self.max_visibles {
            return Err(Error::TooManyVisibles);
        }

        // Unlink all participants
        let tmp = self.participants.keys().cloned().collect::<Vec<_>>();
        for name in tmp {
            self.link_video_to_fakesink(&name)?;
        }

        self.visibles = 0;

        // Link all given participants
        for (n, name) in names.iter().enumerate() {
            self.link_video_to_compositor(name, n)?;
            self.visibles += 1;
        }

        self.layout()?;

        Ok(())
    }

    pub fn layout(&self) -> Result<(), Error> {
        if self.pipeline.current_state() == gst::State::Playing {
            return Err(Error::PlayingPipelineForbidden);
        }

        let count = self.visibles;
        trace!("visibles = {count}");
        self.layout_overlay(
            &self.title,
            self.layout.title_position(count),
            self.layout.title_alignment(),
        );
        self.layout_overlay(
            &self.clock,
            self.layout.clock_position(count),
            self.layout.clock_alignment(),
        );
        self.layout_overlay(
            &self.speaking,
            self.layout.speaking_position(count),
            self.layout.speaking_alignment(count),
        );
        for (n, pad) in self.compositor.sink_pads()[1..].iter().enumerate() {
            let (pos, size, alpha) = if n < count {
                (
                    self.layout.position(n, count),
                    self.layout.size(n, count),
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
    pub fn play(&self) {
        self.pipeline.set_state(gst::State::Playing).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    pub fn pause(&self) {
        self.pipeline.set_state(gst::State::Paused).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    /// wait until mixer generates error or ends
    pub fn run(&self) -> impl FnOnce() {
        let pipeline = self.pipeline.clone();

        move || {
            // wait until error or EOS
            let bus = pipeline.bus().unwrap();
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
            pipeline
                .set_state(gst::State::Null)
                .expect("Unable to set the pipeline to the `Null` state");
        }
    }

    pub fn generate_dot_file(&self, filename_without_extension: &str) {
        if let Ok(path) = std::env::var("GST_DEBUG_DUMP_DOT_DIR") {
            info!(
                "writing DOT file `{}/{filename_without_extension}.dot`...",
                path
            );
            gst::debug_bin_to_dot_file(
                &self.pipeline,
                gst::DebugGraphDetails::ALL,
                filename_without_extension,
            );
        } else {
            warn!("can not write DOT file. You need to set GST_DEBUG_DUMP_DOT_DIR in environment to a absolute path");
        }
    }
}

impl<L, S> Mixer<L, S>
where
    L: Layout,
    S: Source,
{
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

    fn link_audio(&mut self, name: &str) -> Result<(), Error> {
        trace!("linking audio of {:?}...", name);

        let participant = self
            .participants
            .get_mut(name)
            .ok_or_else(|| Error::ParticipantNotFound(name.to_owned()))?;

        let mixer_pad = self.audio_mixer.request_pad_simple("sink_%").unwrap();

        participant.source.audio_src_pad().link(&mixer_pad).unwrap();

        participant.audio_mixer_pad = Some(mixer_pad);

        Ok(())
    }

    /// Unlink names from the audiomixer. Used before destroying the source.
    fn unlink_audio(&mut self, name: &str) -> Result<(), Error> {
        trace!("unlinking audio of {name:?}...");

        let participant = self
            .participants
            .get_mut(name)
            .ok_or_else(|| Error::ParticipantNotFound(name.to_owned()))?;

        if let Some(pad) = participant.audio_mixer_pad.take() {
            participant.source.audio_src_pad().unlink(&pad).unwrap();
        }

        Ok(())
    }

    fn link_video_to_fakesink(&mut self, name: &str) -> Result<(), Error> {
        trace!("linking video of {name:?} to fakesink...");

        let participant = self
            .participants
            .get_mut(name)
            .ok_or_else(|| Error::ParticipantNotFound(name.to_owned()))?;

        match &participant.video_link_status {
            VideoLinkStatus::None => {}
            VideoLinkStatus::Fakesink(_) => return Ok(()),
            VideoLinkStatus::Compositor(_, pad) => {
                participant.source.video_src_pad().unlink(pad).unwrap()
            }
        }

        let fakesink = gst::ElementFactory::make("fakesink", None).unwrap();
        self.pipeline.add(&fakesink).unwrap();
        participant
            .source
            .video_src_pad()
            .link(&fakesink.static_pad("sink").unwrap())
            .unwrap();

        participant.video_link_status = VideoLinkStatus::Fakesink(fakesink);

        Ok(())
    }

    fn link_video_to_compositor(&mut self, name: &str, n: usize) -> Result<(), Error> {
        trace!("linking video of {name:?} to compositor@{n}...");

        let participant = self
            .participants
            .get_mut(name)
            .ok_or_else(|| Error::ParticipantNotFound(name.to_owned()))?;

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

        trace!("linked video of {name:?} to compositor@{n}...");

        Ok(())
    }

    fn unlink_video(&mut self, name: &str) -> Result<(), Error> {
        trace!("unlinking video of {name:?}...");

        let participant = self
            .participants
            .get_mut(name)
            .ok_or_else(|| Error::ParticipantNotFound(name.to_owned()))?;

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

        trace!("unlinked video of {name:?}...");

        Ok(())
    }
}
