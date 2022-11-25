// sub-modules
mod participant;
mod sink;
mod source;

// forward useful sub-module stuff as public
pub use participant::Participant;
pub use sink::Sink;
pub use source::Source;

// what else we need from this lib
use crate::{Alignment, Error, Layout, Position, Size};
use participant::LinkStatus;

// what we need from external libraries
use core::{fmt::Debug, hash::Hash, mem::replace};
use gst::prelude::*;
use std::collections::HashMap;

/// Mixer managing the GStreamer pipeline using the given layout and source type
/// # Types
/// - `L`: Layout to use to compose output picture.
/// - `SRC`: Source type to use when adding participants.
/// - `SINK`: Sink type to use for output.
pub struct Mixer<L, SRC, SINK, ID>
where
    L: Layout,
    SRC: Source,
    SINK: Sink,
    ID: Eq + Ord + Hash + Copy,
{
    /// GStreamer element which composes the output video out of the source videos.
    pub compositor: gst::Element,
    /// GStreamer element which composes the output audio out of the source audios.
    pub audio_mixer: gst::Element,
    /// Maximum number of visible participants.
    pub max_visible: usize,
    /// Number of currently visible participants.
    pub max_hearable: usize,
    /// Number of currently visible participants.
    pub num_visible: usize,
    /// Number of currently hearable participants.
    pub num_hearable: usize,
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
    video_sink_pads: Vec<gst::Pad>,
    /// sink pads participants can attach to transmit audio
    audio_sink_pads: Vec<gst::Pad>,
    /// Current participants.
    pub participants: HashMap<ID, Participant<SRC>>,
    /// Holds the output sink.
    pub output: SINK,
}

impl<L, SRC, SINK, ID> Mixer<L, SRC, SINK, ID>
where
    L: Layout,
    SRC: Source,
    SINK: Sink,
    ID: Eq + Ord + Hash + Copy + Debug,
{
    /// Create a new mixer and setup the initial GStreamer pipeline with the given type of sink.
    /// # Arguments
    /// - `resolution`: Output video resolution.
    /// - `max_visibles`: Maximum number of visible participants.
    /// - `visibles`: Number of currently visible participants.
    pub fn new(
        resolution: Size,
        max_visible: usize,
        max_hearable: usize,
        sink_params: SINK::Parameters,
    ) -> Result<Self, Error<ID>> {
        // get width/height
        let width = resolution.width;
        let height = resolution.height;
        trace!(
            "Output video resolution (WxH): {width}x{height} = {:2}",
            resolution.ratio()
        );

        // create new layout for the given resolution
        let layout = L::new(&resolution);
        // create new GStreamer pipeline
        let pipeline = gst::Pipeline::new(None);

        // create mixer bin
        let bin = gst::parse_bin_from_description(
            &format!(
                r#"
                    videotestsrc
                        name=video-background-src
                        pattern=black
                        is-live=true
                    ! capssetter
                        name=video-caps
                        caps=video/x-raw,format=RGB,width={width},height={height}
                    ! queue
                        name=video-background
                    ! compositor
                        name=video-compositor
                        ignore-inactive-pads=true
                    ! clockoverlay
                        name=video-clock-overlay
                        font-desc=Sans,14
                        time-format="%x %X %Z"
                        xpad=10
                        ypad=2
                        color=0xffffffff
                    ! textoverlay
                        name=video-title-overlay
                        font-desc=Sans,16
                        xpad=10
                        ypad=2
                        color=0xffffffff
                    ! textoverlay
                        name=video-speaking-overlay
                        font-desc=Sans,16
                        xpad=10
                        ypad=2
                        color=0xffffffff
                    ! queue
                        name=video-out
        
                    audiotestsrc
                        name=audio-background
                        is-live=true
                        volume=0.0
                    ! capssetter
                        caps=audio/x-raw,format=S16LE,channels=2,layout=interleaved,rate=48000
                    ! audiomixer
                        name=audio-mixer
                        ignore-inactive-pads=true
                    ! queue
                        name=audio-out
            "#
            ),
            false,
        )
        .unwrap();

        // add bin to pipeline
        pipeline.add(&bin).unwrap();

        // get video elements from bin
        let compositor = bin.by_name("video-compositor").unwrap();
        let clock = bin.by_name("video-clock-overlay").unwrap();
        let title = bin.by_name("video-title-overlay").unwrap();
        let speaking = bin.by_name("video-speaking-overlay").unwrap();
        let video_out = bin.by_name("video-out").unwrap();

        // get audio elements from bin
        let audio_mixer = bin.by_name("audio-mixer").unwrap();
        let audio_out = bin.by_name("audio-out").unwrap();

        // create ghost pad for video output and add it to pipeline
        let video_src_ghostpad =
            gst::GhostPad::with_target(None, &video_out.static_pad("src").unwrap()).unwrap();
        bin.add_pad(&video_src_ghostpad).unwrap();

        // create ghost pad for audio output and add it to pipeline
        let audio_src_ghostpad =
            gst::GhostPad::with_target(None, &audio_out.static_pad("src").unwrap()).unwrap();
        bin.add_pad(&audio_src_ghostpad).unwrap();

        // create output sink
        let output = SINK::new(&pipeline, sink_params);

        // connect output pads to output sinks
        video_src_ghostpad.link(&output.video_sink_pad()).unwrap();
        audio_src_ghostpad.link(&output.audio_sink_pad()).unwrap();

        // prepare enough sink pads at video compositor to take max_visibles video streams
        let mut video_sink_pads = Vec::new();
        for i in 0..max_visible {
            let video_sink_ghostpad = gst::GhostPad::with_target(
                Some(&format!("video_sink_{i}")),
                &compositor.request_pad_simple("sink_%u").unwrap(),
            )
            .unwrap();
            bin.add_pad(&video_sink_ghostpad).unwrap();
            video_sink_pads.push(video_sink_ghostpad.upcast::<gst::Pad>());
        }

        // prepare enough sink pads at audio mixer to take max_hearable audio streams
        let mut audio_sink_pads = Vec::new();
        for i in 0..max_hearable {
            let audio_sink_ghostpad = gst::GhostPad::with_target(
                Some(&format!("audio_sink_{i}")),
                &audio_mixer.request_pad_simple("sink_%u").unwrap(),
            )
            .unwrap();
            bin.add_pad(&audio_sink_ghostpad).unwrap();
            audio_sink_pads.push(audio_sink_ghostpad.upcast::<gst::Pad>());
        }

        Ok(Mixer {
            // remember all those elements and pads
            compositor,
            audio_mixer,
            max_visible,
            max_hearable,
            num_visible: 0,
            num_hearable: 0,
            clock: Some(clock),
            title: Some(title),
            speaking: Some(speaking),
            layout,
            video_sink_pads,
            audio_sink_pads,
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
        id: ID,
        display_name: String,
        params: SRC::Parameters,
    ) -> Result<(), Error<ID>> {
        trace!("add participant( '{display_name}' )");
        // check preconditions
        if self.pipeline.current_state() == gst::State::Playing {
            return Err(Error::PlayingPipelineForbidden);
        }
        if self.participants.contains_key(&id) {
            return Err(Error::IdDoublet(id));
        }

        // add new participant
        let participant = Participant::new(
            &self.pipeline,
            self.layout.resolution(),
            display_name,
            params,
        );
        self.participants.insert(id, participant);

        // link new participant
        self.link_audio(id)?;
        self.link_video_to_fakesink(id)?;

        // re-layout
        self.layout()?;

        Ok(())
    }

    /// remove an once added participant from the mixer.
    /// # Arguments
    /// - `id`: Unique identifier of the participant.
    pub fn remove_participant(&mut self, id: ID) -> Result<(), Error<ID>> {
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
            .remove(&id)
            .ok_or(Error::ParticipantNotFound(id))?;

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
    pub fn set_visibles(&mut self, ids: &[ID]) -> Result<(), Error<ID>> {
        // check preconditions
        if self.pipeline.current_state() == gst::State::Playing {
            return Err(Error::PlayingPipelineForbidden);
        }

        // check if given list exceeds maximum length
        if ids.len() > self.max_visible {
            return Err(Error::TooManyVisibles);
        }

        // Unlink all participants
        for id in self.participants.keys().copied().collect::<Vec<_>>() {
            self.link_video_to_fakesink(id)?;
        }
        self.num_visible = 0;

        // Link all given participants
        for (n, id) in ids.iter().enumerate() {
            self.link_video_to_compositor(*id, n)?;
            self.num_visible += 1;
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
    pub fn play(&mut self) {
        self.pipeline.set_state(gst::State::Playing).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(100));
        self.output.on_play();
    }

    /// pause playing of pipeline
    pub fn pause(&mut self) {
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

    /// Re-layout the current compositor scene.
    fn layout(&self) -> Result<(), Error<ID>> {
        // check preconditions
        if self.pipeline.current_state() == gst::State::Playing {
            return Err(Error::PlayingPipelineForbidden);
        }

        // layout overlays
        self.layout_overlay(
            &self.title,
            self.layout.title_position(self.num_visible),
            self.layout.title_alignment(),
        );
        self.layout_overlay(
            &self.clock,
            self.layout.clock_position(self.num_visible),
            self.layout.clock_alignment(),
        );
        self.layout_overlay(
            &self.speaking,
            self.layout.speaking_position(self.max_visible),
            self.layout.speaking_alignment(self.num_visible),
        );

        // configure compositor sink pads (which might be connected to the participants' sources)
        for (n, pad) in self.compositor.sink_pads()[1..].iter().enumerate() {
            let (pos, size, alpha) = if n < self.num_visible {
                (
                    self.layout.position(n, self.num_visible),
                    self.layout.size(n, self.num_visible),
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
    fn link_audio(&mut self, id: ID) -> Result<(), Error<ID>> {
        trace!("linking audio of {:?}...", id);

        let participant = self
            .participants
            .get_mut(&id)
            .ok_or(Error::ParticipantNotFound(id))?;

        let sink_pads = &mut self.audio_sink_pads;
        let src_pad = &mut participant.source.audio_src_pad();
        let link_status = &mut participant.audio_link_status;

        match &link_status {
            LinkStatus::None => {}
            LinkStatus::Mixer(pad) => {
                src_pad.unlink(pad).unwrap();
            }
            _ => panic!(),
        }

        for sink_pad in sink_pads {
            if !sink_pad.is_linked() {
                src_pad.link(sink_pad).unwrap();
                *link_status = LinkStatus::Mixer(sink_pad.clone());
                trace!("linked audio of {id:?} to audo-mixer.");
                return Ok(());
            }
        }

        Err(Error::CannotLinkAudio(id))
    }

    /// Unlink participant's audio from the audiomixer.
    fn unlink_audio(&mut self, id: ID) -> Result<(), Error<ID>> {
        trace!("unlinking audio of {id:?}...");

        let participant = self
            .participants
            .get_mut(&id)
            .ok_or(Error::ParticipantNotFound(id))?;

        if let Some(pad) = participant.audio_mixer_pad.take() {
            participant.source.audio_src_pad().unlink(&pad).unwrap();
        }

        trace!("unlinked audio of {id:?}...");

        Ok(())
    }

    /// Link participant's video source to fake sink (while it's invisible).
    fn link_video_to_fakesink(&mut self, id: ID) -> Result<(), Error<ID>> {
        trace!("linking video of {id:?} to fakesink...");

        let participant = self
            .participants
            .get_mut(&id)
            .ok_or(Error::ParticipantNotFound(id))?;

        match &participant.video_link_status {
            LinkStatus::None => {}
            LinkStatus::Fakesink(_) => return Ok(()),
            LinkStatus::Compositor(_, pad) => {
                participant.source.video_src_pad().unlink(pad).unwrap()
            }
            _ => panic!(),
        }

        let fakesink = gst::ElementFactory::make_with_name("fakesink", None).unwrap();
        self.pipeline.add(&fakesink).unwrap();
        participant
            .source
            .video_src_pad()
            .link(&fakesink.static_pad("sink").unwrap())
            .unwrap();

        participant.video_link_status = LinkStatus::Fakesink(fakesink);

        Ok(())
    }

    /// Link participant's source to video compositor.
    fn link_video_to_compositor(&mut self, id: ID, n: usize) -> Result<(), Error<ID>> {
        trace!("linking video of {id:?} to compositor@{n}...");

        let participant = self
            .participants
            .get_mut(&id)
            .ok_or(Error::ParticipantNotFound(id))?;

        match &participant.video_link_status {
            LinkStatus::None => {}
            LinkStatus::Fakesink(fakesink) => {
                participant
                    .source
                    .video_src_pad()
                    .unlink(&fakesink.static_pad("sink").unwrap())
                    .unwrap();
                fakesink.set_state(gst::State::Null).unwrap();
                self.pipeline.remove(fakesink).unwrap();
            }
            LinkStatus::Compositor(curr_n, pad) => {
                if *curr_n != n {
                    participant.source.video_src_pad().unlink(pad).unwrap();
                }
            }
            _ => panic!(),
        }

        let compositor_sink_pads = &self.video_sink_pads;

        participant
            .source
            .video_src_pad()
            .link(&compositor_sink_pads[n + 1])
            .unwrap();

        participant.video_link_status =
            LinkStatus::Compositor(n, compositor_sink_pads[n + 1].clone());

        trace!("linked video of {id:?} to compositor@{n}...");

        Ok(())
    }

    // Unlink participant's video source from compositor.
    fn unlink_video(&mut self, id: ID) -> Result<(), Error<ID>> {
        trace!("unlinking video of {id:?}...");

        let participant = self
            .participants
            .get_mut(&id)
            .ok_or(Error::ParticipantNotFound(id))?;

        match replace(&mut participant.video_link_status, LinkStatus::None) {
            LinkStatus::None => {}
            LinkStatus::Fakesink(fakesink) => {
                participant
                    .source
                    .video_src_pad()
                    .unlink(&fakesink.static_pad("sink").unwrap())
                    .unwrap();
                fakesink.set_state(gst::State::Null).unwrap();
                self.pipeline.remove(&fakesink).unwrap();
            }
            LinkStatus::Compositor(_, pad) => {
                participant.source.video_src_pad().unlink(&pad).unwrap();
            }
            _ => panic!(),
        }

        trace!("unlinked video of {id:?}...");

        Ok(())
    }
}

impl<L, SRC, SINK, ID> Drop for Mixer<L, SRC, SINK, ID>
where
    L: Layout,
    SRC: Source,
    SINK: Sink,
    ID: Eq + Ord + Hash + Copy,
{
    /// halt pipeline (can not be played again)
    fn drop(&mut self) {
        trace!("exiting mixer");

        // call sink to prepare for dropping pipeline
        self.output.on_exit(&self.pipeline);

        // stop pipeline
        self.pipeline
            .set_state(gst::State::Null)
            .expect("Unable to set the pipeline to the `Null` state");

        trace!("Mixer exited successfully");
    }
}
