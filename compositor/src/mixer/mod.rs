// sub-modules
mod participant;
mod sink;
mod source;

// forward useful sub-module stuff as public
pub use participant::Participant;
pub use sink::Sink;
pub use source::Source;

// what else we need from this lib
use crate::{layout, Alignment, Error, Layout, Position, Size};
use participant::VideoLinkStatus;

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
    ID: Eq + Ord + Hash + Copy + Debug,
{
    /// GStreamer element which composes the output video out of the source videos.
    pub compositor: gst::Element,
    /// GStreamer element which composes the output audio out of the source audios.
    pub audio_mixer: gst::Element,
    /// Maximum number of visible participants.
    pub max_visible: Option<usize>,
    /// Number of currently visible participants.
    pub visibles: Vec<ID>,
    /// GStreamer element for rendering a clock into the output picture if whished.
    clock: Option<gst::Element>,
    /// GStreamer element for rendering a title into the output picture if whished.
    title: Option<gst::Element>,
    /// GStreamer element for rendering a sub title display into the output picture if whished.
    subtitle: Option<gst::Element>,
    /// The mixer GStreamer pipeline.
    pipeline: gst::Pipeline,
    /// Layout of the output picture.
    layout: L,
    /// Current participants.
    pub participants: HashMap<ID, Participant<SRC>>,
    pub speaker: Option<ID>,
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
    /// - `max_visible`: Maximum number of visible participants.
    /// - `visibles`: Number of currently visible participants.
    pub fn new(
        resolution: Size,
        max_visible: Option<usize>,
        sink_params: SINK::Parameters,
        speaker_mode: layout::SpeakerMode,
    ) -> Result<Self, Error<ID>> {
        // get width/height
        let width = resolution.width;
        let height = resolution.height;
        trace!(
            "Output video resolution (WxH): {width}x{height} = {:2}",
            resolution.ratio()
        );

        // create new layout for the given resolution
        let layout = L::new(resolution, speaker_mode);
        // create new GStreamer pipeline
        let pipeline = gst::parse_launch(&format!(
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
                        zero-size-is-unscaled=true
                    ! clockoverlay
                        name=video-clock-overlay
                        font-desc=Sans,14
                        time-format="%x %X %Z"
                        xpad=10
                        ypad=2
                        color=0xffffffff
                    ! textoverlay
                        name=video-title-overlay
                        font-desc="Helvetica Bold 25"
                        xpad=10
                        ypad=2
                        color=0xffffffff
                    ! textoverlay
                        name=video-subtitle-overlay
                        font-desc="Helvetica Bold 25"
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
                    ! queue
                    ! audiomixer
                        name=audio-mixer
                        ignore-inactive-pads=true
                    ! queue
                        name=audio-out
            "#
        ))
        .expect("can not create pipeline");

        let pipeline = pipeline
            .downcast::<gst::Pipeline>()
            .expect("not a pipeline");

        // get video elements from bin
        let compositor = pipeline.by_name("video-compositor").unwrap();
        let clock = pipeline.by_name("video-clock-overlay").unwrap();
        let title = pipeline.by_name("video-title-overlay").unwrap();
        let subtitle = pipeline.by_name("video-subtitle-overlay").unwrap();
        let video_out = pipeline.by_name("video-out").unwrap();
        let video_output_pad = video_out.static_pad("src").unwrap();

        // get audio elements from bin
        let audio_mixer = pipeline.by_name("audio-mixer").unwrap();
        let audio_out = pipeline.by_name("audio-out").unwrap();
        let audio_output_pad = audio_out.static_pad("src").unwrap();

        // create output sink
        let output = SINK::new(&pipeline, sink_params);

        // connect output pads to output sinks
        video_output_pad.link(&output.video_sink_pad()).unwrap();
        audio_output_pad.link(&output.audio_sink_pad()).unwrap();

        Ok(Mixer {
            // remember all those elements and pads
            compositor,
            audio_mixer,
            max_visible,
            visibles: Vec::new(),
            clock: Some(clock),
            title: Some(title),
            subtitle: Some(subtitle),
            layout,
            pipeline,
            participants: HashMap::new(),
            speaker: None,
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
        debug!("add participant( '{display_name}' ({id:?})");
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

        if let Some(max_visible) = self.max_visible {
            // Show visibles if there is unused space
            if self.visibles.len() < max_visible {
                debug!("automatically making participant {id:?} visible because there are unused visible ports");
                // get currently visible participants
                let mut visibles = self.visibles.clone();
                // make new participant visible
                visibles.push(id);
                // update visibles
                self.set_visibles(&visibles).unwrap();
            }
        }
        // re-layout
        self.layout()?;

        Ok(())
    }

    /// remove an once added participant from the mixer.
    /// # Arguments
    /// - `id`: Unique identifier of the participant.
    pub fn remove_participant(&mut self, remove_id: ID) -> Result<(), Error<ID>> {
        debug!("remove participant {remove_id:?}");

        // check preconditions
        if self.pipeline.current_state() == gst::State::Playing {
            return Err(Error::PlayingPipelineForbidden);
        }

        // unlink participant from rest of the pipeline
        self.unlink_audio(remove_id)?;
        self.unlink_video(remove_id)?;

        // remove participant from stored participants
        let participant = self
            .participants
            .remove(&remove_id)
            .ok_or(Error::ParticipantNotFound(remove_id))?;

        participant.source.remove(&self.pipeline);

        // check if participant is visible
        if let Some(pos) = self.visibles.iter().position(|i| i == &remove_id) {
            self.visibles.remove(pos);
        }

        if let Some(max_visible) = self.max_visible {
            // fill up visibles with invisible participants
            if self.visibles.len() < max_visible {
                // clone currently visible participants to make a new list
                let mut visibles = self.visibles.clone();
                // add all participants to this list which are invisible and not the removed one
                for id in self.participants.keys() {
                    if !self.visibles.contains(id) {
                        visibles.push(*id);
                        // stop if we reach max_visible
                        if visibles.len() == max_visible {
                            break;
                        }
                    }
                }
                debug!(
                "automatically filling up visibles with former invisible participants {visibles:?}"
            );
                // update visibles
                self.set_visibles(&visibles).unwrap();
            }
        }

        // re-layout
        self.layout()?;

        Ok(())
    }

    /// Select the participants which are visible.
    /// All previously visible participants get invisible if they are not in the list.
    /// See set_speaker() for further info about how the order will be interpreted.
    /// # Arguments
    /// - `ids`: List of identifiers of participants which shall get visible
    pub fn set_visibles(&mut self, ids: &[ID]) -> Result<(), Error<ID>> {
        debug!("set visibles: {:?} -> {:?}", self.visibles, ids);

        // check preconditions
        if self.pipeline.current_state() == gst::State::Playing {
            return Err(Error::PlayingPipelineForbidden);
        }

        if let Some(max_visible) = self.max_visible {
            // check if given list exceeds maximum length
            if ids.len() > max_visible {
                return Err(Error::TooManyVisibles);
            }
        }

        // Unlink all participants
        for id in self.visibles.clone().iter().collect::<Vec<_>>() {
            self.link_video_to_fakesink(*id)?;
        }

        // Link all given participants
        for id in ids {
            self.link_video_to_compositor(*id)?;
        }

        // copy ID list of visibles
        self.visibles = ids.into();

        // re-layout
        self.layout()?;

        Ok(())
    }

    /// Sets the current speaker
    /// Visualization of the current speaker depends on the layout's speaker mode
    /// # Arguments
    /// - `id`: ID of the participant to mark as speaker
    /// # Speaker modes
    /// Depending on the layout::SpeakerMode set in the layout the speaker might be moved into or within the visibles.
    pub fn set_speaker(&mut self, speaker_id: Option<ID>) -> Result<(), Error<ID>> {
        debug!("set speaker {:?}...", speaker_id);

        if let Some(speaker_id) = &speaker_id {
            let mut visibles = self.visibles.clone();

            // check if speaker is participant
            if !self.participants.contains_key(speaker_id) {
                error!("speaker must be a participant");
            }
            use layout::*;
            match self.layout.speaker_mode() {
                SpeakerMode::FirstShift => {
                    // check if speaker is in visibles
                    match visibles.iter().position(|id| id == speaker_id) {
                        Some(pos) => {
                            trace!("remove visible at {pos}");
                            // remove speaker from visibles
                            visibles.remove(pos);
                        }
                        None => {
                            if let Some(max_visible) = self.max_visible {
                                // remove last visible if visibles are filled completely
                                if visibles.len() == max_visible {
                                    trace!("remove last visible");
                                    visibles.pop();
                                }
                            }
                        }
                    }
                    trace!("insert speaker {:?} at 0", *speaker_id);
                    // insert speaker at first
                    visibles.insert(0, *speaker_id);
                }
                SpeakerMode::FirstSwap => {
                    // check if speaker is in visibles
                    match visibles.iter().position(|id| id == speaker_id) {
                        Some(pos) => {
                            trace!("swap visible 0 and {pos}");
                            // swap with previous speaker
                            visibles.swap(0, pos);
                        }
                        None => {
                            if let Some(max_visible) = self.max_visible {
                                // remove last visible if visibles are filled completely
                                if visibles.len() == max_visible {
                                    trace!("remove last visible");
                                    visibles.pop();
                                }
                            }
                            // insert speaker at first
                            trace!("insert speaker {:?} at 0", *speaker_id);
                            visibles.insert(0, *speaker_id);
                        }
                    }
                }
                _ => (),
            }
            self.speaker = Some(*speaker_id);
            self.set_visibles(&visibles)?;
            self.layout()?;
        }
        Ok(())
    }

    /// set the title text within the mixer view if provided
    pub fn set_title(&self, text: &str) {
        debug!("set title {text}");

        if let Some(title) = &self.title {
            title.set_property("text", text);
        }
    }

    /// set the sub title text within the mixer view if provided
    pub fn set_subtitle(&self, text: &str) {
        debug!("set subtitle {text}");

        if let Some(subtitle) = &self.subtitle {
            subtitle.set_property("text", text);
        }
    }

    /// start playing of pipeline
    pub fn play(&mut self) {
        debug!("start playing");
        self.pipeline.set_state(gst::State::Playing).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(100));
        self.output.on_play();
    }

    /// pause playing of pipeline
    pub fn pause(&mut self) {
        debug!("pause playing");
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

        // get number of visible participants
        let num_visible = self.visibles.len();

        // layout overlays
        self.layout_overlay(
            &self.title,
            self.layout.title_position(num_visible),
            self.layout.title_alignment(),
        );
        self.layout_overlay(
            &self.clock,
            self.layout.clock_position(num_visible),
            self.layout.clock_alignment(),
        );

        self.layout_overlay(
            &self.subtitle,
            self.layout.subtitle_position(num_visible),
            self.layout.subtitle_alignment(num_visible),
        );

        // configure compositor sink pads (which might be connected to the participants' sources)
        for (n, pad) in self.compositor.sink_pads()[1..].iter().enumerate() {
            let (pos, size, alpha) = if n < num_visible {
                (
                    self.layout.position(n, num_visible),
                    self.layout.size(n, num_visible),
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

        let mixer_pad = self.audio_mixer.request_pad_simple("sink_%").unwrap();

        participant.source.audio_src_pad().link(&mixer_pad).unwrap();

        participant.audio_mixer_pad = Some(mixer_pad);

        Ok(())
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
            VideoLinkStatus::None => {}
            VideoLinkStatus::Fakesink(_) => {
                warn!("trying to link participant {id:?} to fakesink when it is already linked");
                return Ok(());
            }
            VideoLinkStatus::Compositor(pad) => {
                participant.source.video_src_pad().unlink(pad).unwrap();
                self.compositor.release_request_pad(pad);
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
    fn link_video_to_compositor(&mut self, id: ID) -> Result<(), Error<ID>> {
        trace!("linking video of {id:?} to compositor...");

        let participant = self
            .participants
            .get_mut(&id)
            .ok_or(Error::ParticipantNotFound(id))?;

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
            VideoLinkStatus::Compositor(_) => {
                warn!("trying to link participant {id:?} to compositor when it is already linked");
                return Ok(());
            }
        }

        trace!("creating compositor sink for participant {id:?}");
        let pad = self
            .compositor
            .request_pad_simple("sink_%u")
            .expect("cannot create sink pad");
        pad.set_property_from_str("sizing-policy", "keep-aspect-ratio");
        participant.source.video_src_pad().link(&pad).unwrap();
        participant.video_link_status = VideoLinkStatus::Compositor(pad);

        trace!("successfully linked video of {id:?} to compositor.");

        Ok(())
    }

    // Unlink participant's video source from compositor.
    fn unlink_video(&mut self, id: ID) -> Result<(), Error<ID>> {
        trace!("unlinking video of {id:?}...");

        let participant = self
            .participants
            .get_mut(&id)
            .ok_or(Error::ParticipantNotFound(id))?;

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
            VideoLinkStatus::Compositor(pad) => {
                participant.source.video_src_pad().unlink(&pad).unwrap();
                self.compositor.release_request_pad(&pad);
            }
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
    ID: Eq + Ord + Hash + Copy + Debug,
{
    /// halt pipeline (can not be played again)
    fn drop(&mut self) {
        trace!("exiting mixer");

        if self.pipeline.current_state() == gst::State::Paused {
            self.pipeline.set_state(gst::State::Playing).unwrap();
        }

        // call sink to prepare for dropping pipeline
        self.output.on_exit(&self.pipeline);

        self.pause();

        for id in self.participants.keys().cloned().collect::<Vec<_>>() {
            let _ = self.remove_participant(id);
        }

        // stop pipeline
        self.pipeline
            .set_state(gst::State::Null)
            .expect("Unable to set the pipeline to the `Null` state");

        trace!("Mixer exited successfully");
    }
}
