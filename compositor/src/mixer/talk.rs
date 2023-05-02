use anyhow::Result;

use crate::*;
use core::{
    fmt::{Debug, Display},
    hash::Hash,
};
use std::collections::HashMap;

/// sub stream ID for testing purposes.
#[allow(dead_code)]
#[derive(Debug, Hash, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum MediaSessionType {
    /// participant's picture (default)
    #[default]
    Camera,
    /// participant's screen share
    ScreenCapture,
}

impl std::fmt::Display for MediaSessionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MediaSessionType::Camera => write!(f, "Camera"),
            MediaSessionType::ScreenCapture => write!(f, "Screen"),
        }
    }
}

/// Stream ID consisting of one main ID and a sub ID for participants having multiple streams (e.g. Camera and Slides)
#[derive(Debug, Hash, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct StreamId<ID>
where
    ID: Eq + Ord + Hash + Copy + Debug + Display,
{
    /// ID identifying the participant
    pub id: ID,
    /// sub ID identifying the stream of the participant
    pub stream: MediaSessionType,
}

impl<ID> StreamId<ID>
where
    ID: Eq + Ord + Hash + Copy + Debug + Display,
{
    pub fn default(id: ID) -> Self {
        Self {
            id,
            stream: Default::default(),
        }
    }
    pub fn camera(id: ID) -> Self {
        Self {
            id,
            stream: MediaSessionType::Camera,
        }
    }
    pub fn screen(id: ID) -> Self {
        Self {
            id,
            stream: MediaSessionType::ScreenCapture,
        }
    }
}

impl<ID> From<ID> for StreamId<ID>
where
    ID: Eq + Ord + Hash + Copy + Debug + Display,
{
    fn from(id: ID) -> Self {
        StreamId::default(id)
    }
}

impl<ID> std::fmt::Display for StreamId<ID>
where
    ID: Eq + Ord + Hash + Copy + Debug + Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#{id} ({stream})", id = self.id, stream = self.stream)
    }
}

impl<ID> StreamId<ID>
where
    ID: Eq + Ord + Hash + Copy + Debug + Display,
{
    pub fn new(id: ID, stream: MediaSessionType) -> Self {
        Self { id, stream }
    }
}

/// A talk consisting of participants and managing maximum amount of visibles.
///
/// # Types
///
/// - `SRC`: Source type which will be created when adding a participant.
/// - `SINK`: Sink type which will be created.
/// - `ID`: Type which can identify a stream.
///
pub struct Talk<SRC, SINK, ID>
where
    SRC: crate::Source,
    SRC::Parameters: Debug,
    SINK: crate::Sink,
    SINK::Parameters: Debug,
    ID: Eq + Ord + Hash + Copy + Display + Debug + Sync + Send,
{
    #[cfg(test)]
    /// Underlying A/V mixer provided to tests.
    mixer: crate::Mixer<SRC, SINK, StreamId<ID>>,
    #[cfg(not(test))]
    /// Underlying A/V mixer.
    mixer: crate::Mixer<SRC, SINK, StreamId<ID>>,
    /// Maximum number of visible participants in layouts.
    max_visibles: Option<usize>,
    /// Display names that will appear in output video
    names: HashMap<StreamId<ID>, String>,
}

impl<SRC, SINK, ID> Talk<SRC, SINK, ID>
where
    SRC: crate::Source,
    SRC::Parameters: Debug,
    SINK: crate::Sink,
    SINK::Parameters: Debug,
    ID: Eq + Ord + Hash + Copy + Display + Debug + Sync + Send,
{
    /// Create new Talk.
    ///
    /// # Arguments
    ///
    /// - `resolution`: Output video resolution.
    /// - `sink_params`: Parameters to create the output sink.
    /// - `max_visibles`: Maximum number of currently visible streams.
    ///
    pub fn new(
        resolution: Size,
        sink_params: SINK::Parameters,
        max_visibles: Option<usize>,
    ) -> Result<Self> {
        debug!("Starting a new talk...");
        trace!("new( {resolution:?}, {sink_params:?}, {max_visibles:?} )");

        Ok(Self {
            mixer: crate::Mixer::<SRC, SINK, StreamId<ID>>::new(resolution, sink_params)?,
            max_visibles,
            names: HashMap::new(),
        })
    }

    /// Add participant with an initial stream.
    ///
    /// # Arguments
    ///
    /// - `id`: Identifies the stream to add
    /// - 'display_name': Human readable name which might get visible within output composite
    /// - `params`: Proprietary parameters to use when creating sink instance.
    ///
    pub fn add_participant(
        &mut self,
        id: StreamId<ID>,
        display_name: String,
        params: SRC::Parameters,
        initial: StreamStatus,
    ) -> Result<()>
    where
        SRC::Parameters: Debug,
    {
        info!("adding participant {id} ('{display_name}'), {params:?}, {initial} )");

        self.add_stream(id, display_name, params, initial)
    }

    /// Add another stream with the given number to an existing participant.
    ///
    /// # Arguments
    ///
    /// - `id`: Identifies the stream to add
    /// - 'display_name': Human readable name which might get visible within output composite
    /// - `params`: Proprietary parameters to use when creating sink instance.
    /// - `initial`: Initial A/V display status.
    ///
    pub fn add_stream(
        &mut self,
        id: StreamId<ID>,
        display_name: String,
        params: SRC::Parameters,
        initial: StreamStatus,
    ) -> Result<()>
    where
        SRC::Parameters: Debug,
    {
        trace!("add_stream( {id}, '{display_name}', {params:?}, {initial} )");

        // forward to mixer
        self.mixer.add_stream(id, display_name.clone(), params)?;

        // remember display name
        self.names.insert(id, display_name.clone());

        // add display name to video
        self.mixer.insert_source_overlay(
            &id,
            TextOverlay::new(
                &display_name,
                TextFormat {
                    color: Color {
                        r: 0xff,
                        g: 0xff,
                        b: 0xff,
                        a: 0x80,
                    },
                    ..Default::default()
                },
            )
            .into(),
        )?;

        // link audio
        if initial.has_audio {
            self.mixer.link_audio(&id)?;
        }

        Ok(())
    }

    /// check if stream with `id` has video and show it in visibles if slots are left.
    fn maybe_show(&mut self, id: StreamId<ID>) -> Result<()> {
        // if there is a limit show participant only if this is not reached already
        let make_visible = if let Some(max_visibles) = self.max_visibles {
            self.mixer.visibles.len() < max_visibles
        } else {
            true
        };

        // maybe show video stream initially
        if make_visible {
            debug!("showing video stream {id}");
            // get currently visible streams
            let mut visibles = self.mixer.visibles.clone();
            // make new stream visible
            visibles.push(id);
            // update visibles
            self.mixer.set_visibles(&visibles)?;
        }

        Ok(())
    }

    /// check if stream with `id` is visible but has no video and hide it then.
    fn maybe_hide(&mut self, id: StreamId<ID>) -> Result<()> {
        // check if visible but without video
        if self.mixer.is_visible(&id) && !self.mixer.has_video(&id)? {
            debug!("hiding video stream {id}");
            // get currently visible streams
            let mut visibles = self.mixer.visibles.clone();
            // make new stream visible
            visibles.remove(
                visibles
                    .iter()
                    .position(|i| i == &id)
                    .unwrap_or_else(|| panic!("given stream id ({id}) cannot be found")),
            );
            // update visibles
            self.mixer.set_visibles(&visibles)?;
        }

        Ok(())
    }
    /// Remove a stream by stream ID.
    ///
    /// # Arguments
    ///
    /// - `id`: Describes which stream shall be removed.
    ///
    pub fn remove_stream(&mut self, id: StreamId<ID>) -> Result<()> {
        trace!("remove_stream( {id} )");
        // remove from visibles
        self.maybe_hide(id)?;
        // remove name
        self.names.remove(&id);
        // forward to mixer
        self.mixer.remove_stream(id)
    }

    /// Check if a given stream ID is known by the mixer.
    ///
    /// # Arguments
    ///
    /// - `id`: Describes which stream to search for.
    ///
    pub fn contains_stream(&self, id: &StreamId<ID>) -> bool {
        // forward to mixer
        self.mixer.streams.contains_key(id)
    }

    pub fn source_mut(&mut self, id: &StreamId<ID>) -> Option<&mut Stream<SRC>> {
        // forward to mixer
        self.mixer.streams.get_mut(id)
    }

    /// Add a text overlay behind the video compositor.
    ///
    /// # Arguments
    ///
    /// - `text`: Text to display
    /// - `text_format`: Formatting attributes
    ///
    pub fn insert_overlay_text(
        &mut self,
        text: &str,
        text_format: TextFormat,
    ) -> Result<TextOverlay> {
        trace!("push_overlay_text( {text}, {text_format} )");

        // prepare text overlay and add to mixer
        let overlay = TextOverlay::new(text, text_format);
        self.insert_overlay(Overlay::Text(overlay.clone()))?;
        Ok(overlay)
    }

    /// Add a clock clock behind the video compositor.
    ///
    /// # Arguments
    ///
    /// - `text`: Text to display
    /// - `text_format`: Formatting attributes
    ///
    pub fn insert_overlay_clock(
        &mut self,
        format: &str,
        text_format: TextFormat,
    ) -> Result<ClockOverlay> {
        trace!("push_overlay_clock( {format:?}, {text_format:?} )");

        // prepare text overlay and add to mixer
        let overlay = ClockOverlay::new(format, text_format);
        self.insert_overlay(Overlay::Clock(overlay.clone()))?;
        Ok(overlay)
    }

    /// Add an overlay behind the video compositor.
    ///
    /// # Arguments
    ///
    /// - `overlay`: Overlay to insert.
    ///
    fn insert_overlay(&mut self, overlay: crate::Overlay) -> Result<()> {
        // forward to mixer
        self.mixer.insert_overlay(overlay)
    }

    /// Add a text overlay behind the a given source.
    ///
    /// # Arguments
    ///
    /// - `text`: Text to display
    /// - `text_format`: Formatting attributes
    ///
    pub fn insert_source_overlay_text(
        &mut self,
        id: &StreamId<ID>,
        text: &str,
        text_format: TextFormat,
    ) -> Result<TextOverlay> {
        trace!("push_overlay_text( {text:?}, {text_format:?} )");

        // prepare text overlay and add to mixer
        let overlay = TextOverlay::new(text, text_format);
        self.insert_source_overlay(id, Overlay::Text(overlay.clone()))?;
        Ok(overlay)
    }

    /// Add an overlay behind the a given source.
    ///
    /// # Arguments
    ///
    /// - `overlay`: Overlay to insert.
    ///
    fn insert_source_overlay(&mut self, id: &StreamId<ID>, overlay: crate::Overlay) -> Result<()> {
        // forward to mixer
        self.mixer.insert_source_overlay(id, overlay)
    }

    /// Set which participant will be visualized as speaker.
    ///
    /// # Arguments
    ///
    /// - `speaker`: Stream of the speaker or `None`.
    /// - `mode`: How the speaker comes into the scene.
    ///
    pub fn set_speaker(
        &mut self,
        speaker: Option<StreamId<ID>>,
        mode: &SpeakerSwitchMode,
    ) -> Result<()> {
        info!("set_speaker( {speaker:?}, {mode:?} )");

        // we may need to change the visibles if speaker is not visible already
        let mut visibles = self.mixer.visibles.clone();

        if let Some(speaker) = &speaker {
            // check if speaker is stream
            if !self.contains_stream(speaker) {
                error!("speaker must be a stream");
            }

            match mode {
                SpeakerSwitchMode::FirstShift => {
                    // check if speaker is already visible
                    match visibles.iter().position(|id| *id == *speaker) {
                        Some(pos) => {
                            trace!("remove visible speaker at position {pos}");
                            visibles.remove(pos);
                        }
                        None => {
                            if let Some(max_visible) = self.max_visibles {
                                // remove last visible if visibles are filled completely
                                if visibles.len() == max_visible {
                                    trace!("remove last visible");
                                    visibles.pop();
                                }
                            }
                        }
                    }
                    trace!("insert speaker {:?} at position 0", speaker);
                    visibles.insert(0, *speaker);
                }
                SpeakerSwitchMode::FirstSwap => {
                    // check if speaker is in visibles
                    match visibles.iter().position(|id| id == speaker) {
                        Some(pos) => {
                            trace!("swap visible at position 0 with new speaker at position {pos}");
                            visibles.swap(0, pos);
                        }
                        None => {
                            if let Some(max_visible) = self.max_visibles {
                                // remove last visible if visibles are filled completely
                                if visibles.len() == max_visible {
                                    trace!("remove last visible");
                                    visibles.pop();
                                }
                            }
                            // insert speaker at first
                            trace!("insert speaker {:?} at position 0", speaker);
                            visibles.insert(0, *speaker);
                        }
                    }
                }
                _ => (),
            }
            // update visibles
            self.mixer.set_visibles(&visibles)?;
        } else {
            warn!("setting no speaker currently ignored")
        }
        Ok(())
    }

    /// Set status of stream with `id`.
    pub fn set_status(&mut self, id: &StreamId<ID>, new_status: StreamStatus) -> Result<()> {
        info!("set_status({id}, {new_status:?}");

        // remember current video status
        let has_video = new_status.has_video;

        // maybe show or hide if necessary
        if !has_video {
            self.maybe_hide(*id)?;
        }

        // update video status
        self.mixer.set_status(id, new_status)?;

        // maybe show or hide if necessary
        if has_video {
            self.maybe_show(*id)?;
        }

        Ok(())
    }

    /// Apply given layout `L`.
    pub fn layout<L>(&mut self) -> Result<()>
    where
        L: Layout,
    {
        // forward to mixer^
        self.mixer.layout::<L>()
    }

    /// Get mutable access to a source specified by stream ID.
    ///
    /// # Arguments
    ///
    /// - `id`: Describes which stream shall be returned.
    ///
    pub fn get_source(&mut self, id: &StreamId<ID>) -> Option<&mut SRC> {
        if let Some(participant) = self.mixer.streams.get_mut(id) {
            Some(&mut participant.source)
        } else {
            None
        }
    }
    /// generate DOT file of the current pipeline
    ///
    /// # Arguments
    ///
    /// - `filename_without_extension`: Filename without extension.
    /// - `details`: Details of graph.
    ///
    pub fn dot(&self, filename_without_extension: &str, params: &debug::Params) {
        self.mixer.dot(filename_without_extension, params)
    }
}

impl<SRC, SINK, ID> Drop for Talk<SRC, SINK, ID>
where
    SRC: crate::Source,
    SRC::Parameters: Debug,
    SINK: crate::Sink,
    SINK::Parameters: Debug,
    ID: Eq + Ord + Hash + Copy + Display + Debug + Sync + Send,
{
    fn drop(&mut self) {
        debug!("Stopped Talk")
    }
}
/// Mode in which the current speaker shall be displayed.
#[derive(Debug, Clone)]
pub enum SpeakerSwitchMode {
    /// Do not visualize who speaks
    None,
    /// Put the current speaker in front of all others and shift the remaining visible participants down.
    /// If the maximum of visibles is reached and speaker was not visible before the last visible will be shifted out.
    FirstShift,
    /// Put the current speaker in front of all others and if the speaker was visible before swap it with the previous speaker.
    /// If the maximum of visibles is reached and< speaker was not visible before the last visible will be shifted out.
    FirstSwap,
}
