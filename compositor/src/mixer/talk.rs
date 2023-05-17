use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::*;
use core::{
    fmt::{Debug, Display},
    hash::Hash,
};
use std::collections::HashMap;

pub fn media_types() -> impl DoubleEndedIterator<Item = MediaSessionType> {
    // order is priority for set speaker (first available will get focus)

    [MediaSessionType::ScreenCapture, MediaSessionType::Camera].into_iter()
}

/// sub stream ID for testing purposes.
#[allow(dead_code)]
#[derive(
    Debug, Hash, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default,
)]
pub enum MediaSessionType {
    /// participant's picture (default)
    #[default]
    #[serde(rename = "video")]
    Camera,
    /// participant's screen share
    #[serde(rename = "screen")]
    ScreenCapture,
}

impl Display for MediaSessionType {
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

impl<ID> Display for StreamId<ID>
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
#[derive(Debug)]
pub struct Talk<SRC, ID>
where
    SRC: Source,
    SRC::Parameters: Debug,
    ID: Eq + Ord + Hash + Copy + Display + Debug + Sync + Send,
{
    #[cfg(test)]
    /// Underlying A/V mixer provided to tests.
    mixer: Mixer<SRC, StreamId<ID>>,
    #[cfg(not(test))]
    /// Underlying A/V mixer.
    mixer: Mixer<SRC, StreamId<ID>>,
    /// Maximum number of visible participants in layouts.
    max_visibles: Option<usize>,
    /// Display names that will appear in output video
    names: HashMap<StreamId<ID>, String>,
    /// Will be used to cache a speaker whose stream was not added yet
    unknown_speaker: Option<(ID, SpeakerSwitchMode)>,
    speaker: Option<ID>,
}

impl<SRC, ID> Talk<SRC, ID>
where
    SRC: Source,
    SRC::Parameters: Debug,
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
        sink_builder: Box<dyn SinkBuilder>,
        max_visibles: Option<usize>,
    ) -> Result<Self> {
        debug!("Starting a new talk...");
        trace!("new( {resolution:?}, {max_visibles:?} )");

        Ok(Self {
            mixer: Mixer::<SRC, StreamId<ID>>::new(resolution, sink_builder)?,
            max_visibles,
            names: HashMap::new(),
            unknown_speaker: None,
            speaker: None,
        })
    }

    /// Add a stream with the given ID and media type
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
        display_name: &str,
        params: SRC::Parameters,
        initial: StreamStatus,
    ) -> Result<()>
    where
        SRC::Parameters: Debug,
    {
        trace!("add_stream( {id}, '{display_name}', {params:?}, {initial} )");

        // forward to mixer
        self.mixer
            .add_stream(id, display_name.to_string(), params)?;

        // remember display name
        self.names.insert(id, display_name.to_string());

        // add display name to video
        self.mixer.insert_source_overlay(
            &id,
            TextOverlay::new(
                display_name,
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

        // check if the added stream was set as speaker before
        if let Some((speaker, mode)) = self.unknown_speaker.clone() {
            if speaker == id.id {
                // set added stream to speaker now
                self.set_speaker(Some(speaker), &mode)?;
                self.unknown_speaker = None;
            }
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

    pub fn contains_any_stream(&self, id: &ID) -> bool {
        media_types().any(|media_type| self.contains_stream(&StreamId::new(*id, media_type)))
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
    fn insert_overlay(&mut self, overlay: Overlay) -> Result<()> {
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
    fn insert_source_overlay(&mut self, id: &StreamId<ID>, overlay: Overlay) -> Result<()> {
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
    pub fn set_speaker(&mut self, speaker: Option<ID>, mode: &SpeakerSwitchMode) -> Result<()> {
        info!("set_speaker( {speaker:?}, {mode:?} )");

        // we may need to change the visibles if speaker is not visible already
        let mut visibles = self.mixer.visibles.clone();

        // reset any previous memorized speaker (which wasn't available before)
        self.unknown_speaker = None;

        if let Some(speaker) = &speaker {
            // check if speaker is stream
            if !self.contains_any_stream(speaker) {
                debug!("unknown speaker is remembered to activate later");
                self.unknown_speaker = Some((*speaker, mode.clone()));
                return Ok(());
            }

            match mode {
                SpeakerSwitchMode::FirstShift => {
                    // remove all speaker's streams
                    while let Some(pos) = visibles.iter().position(|id| id.id == *speaker) {
                        trace!(
                            "remove {speaker} ({media_type}) from position {pos}",
                            media_type = visibles[pos].stream
                        );
                        visibles.remove(pos);
                    }
                    // make all available media of the speaker visible
                    for media_type in media_types() {
                        let stream = StreamId::new(*speaker, media_type);
                        if self.contains_stream(&stream) {
                            trace!("insert new speaker {speaker} ({media_type}) at position 0");
                            visibles.insert(0, stream);
                        }
                    }
                }
                SpeakerSwitchMode::FirstSwap => {
                    let streams_ids = media_types()
                        .rev()
                        .map(|media_type| StreamId::new(*speaker, media_type));

                    for stream in streams_ids {
                        // check if this speaker stream is visible
                        match visibles.iter().position(|id| *id == stream) {
                            Some(pos) => {
                                trace!(
                                    "swap visible at position 0 with new speaker at position {pos}"
                                );
                                visibles.swap(0, pos);
                            }
                            None => {
                                if self.contains_stream(&stream) {
                                    // insert speaker at first
                                    trace!("insert speaker {speaker} at position 0");
                                    visibles.insert(0, stream);
                                }
                            }
                        }
                    }
                }
                _ => (),
            }

            // remove last visibles if visibles are too many
            if let Some(max_visible) = self.max_visibles {
                while visibles.len() > max_visible {
                    trace!("remove last visible");
                    visibles.pop();
                }
            }

            // update visibles
            self.mixer.set_visibles(&visibles)?;
        } else {
            warn!("setting no speaker currently ignored")
        }
        self.speaker = speaker;

        Ok(())
    }

    pub fn get_speaker(&self) -> Option<ID> {
        self.speaker
    }

    /// Set status of stream with `id`.
    pub fn set_status(&mut self, id: &StreamId<ID>, new_status: StreamStatus) -> Result<()> {
        info!("set_status({id}, {new_status:?}");

        // update video status
        self.mixer.set_status(id, new_status.clone())?;

        // if video was turned on make it visible
        if new_status.has_video {
            self.show(id)?
        }
        Ok(())
    }

    pub fn show(&mut self, id: &StreamId<ID>) -> Result<()> {
        let max_visibles = self.max_visibles.unwrap_or(usize::MAX);
        if self.mixer.visibles.len() < max_visibles {
            self.mixer.show(id)?
        }
        Ok(())
    }

    /// Return `true`, if stream is currently visible
    pub fn is_any_visible(&self, id: &ID) -> bool {
        media_types().any(|media_type| self.mixer.is_visible(&StreamId::new(*id, media_type)))
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
        self.mixer
            .streams
            .get_mut(id)
            .map(|participant| &mut participant.source)
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

impl<SRC, ID> Drop for Talk<SRC, ID>
where
    SRC: Source,
    SRC::Parameters: Debug,
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
