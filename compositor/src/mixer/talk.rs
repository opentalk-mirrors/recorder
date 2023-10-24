//! Talk manages a conference recording.

use anyhow::Result;
use core::{
    fmt::{Debug, Display},
    hash::Hash,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::*;

/// return available media types
pub fn media_types() -> impl DoubleEndedIterator<Item = MediaSessionType> {
    // order is priority for set speaker (first available will get focus)

    [MediaSessionType::ScreenCapture, MediaSessionType::Camera].into_iter()
}

/// sub stream ID for testing purposes.
#[allow(dead_code)]
#[derive(Debug, Hash, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MediaSessionType {
    /// participant's picture (default)
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

/// Stream ID consisting of one participant ID and a stream type.
///
/// # Types
///
/// - `ID`: Type which can identify a participant.
///
#[derive(Debug, Hash, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct StreamId<ID>
where
    ID: Eq + Ord + Hash + Copy + Debug + Display,
{
    /// ID identifying the participant
    pub id: ID,
    /// Type of the stream.
    pub media_type: MediaSessionType,
}

impl<ID> StreamId<ID>
where
    ID: Eq + Ord + Hash + Copy + Debug + Display,
{
    /// Create an ID of the given participant's camera stream.
    ///
    /// # Arguments
    ///
    /// - `id`: ID of the participant
    ///
    pub fn camera(id: ID) -> Self {
        Self {
            id,
            media_type: MediaSessionType::Camera,
        }
    }
    /// Create an ID of the given participant's screen sharing stream.
    ///
    /// # Arguments
    ///
    /// - `id`: ID of the participant
    ///
    pub fn screen(id: ID) -> Self {
        Self {
            id,
            media_type: MediaSessionType::ScreenCapture,
        }
    }
}

impl<ID> Display for StreamId<ID>
where
    ID: Eq + Ord + Hash + Copy + Debug + Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "#{id} ({stream})",
            id = self.id,
            stream = self.media_type
        )
    }
}

impl<ID> StreamId<ID>
where
    ID: Eq + Ord + Hash + Copy + Debug + Display,
{
    /// create new stream ID
    ///
    /// # Arguments
    ///
    /// - `id`: ID of the participant
    /// - `stream`: type of the stream
    ///
    pub fn new(id: ID, stream: MediaSessionType) -> Self {
        Self {
            id,
            media_type: stream,
        }
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
    SRC: Source + Debug,
    SRC::Parameters: Debug,
    ID: Eq + Ord + Hash + Copy + Display + Debug + Sync + Send,
{
    /// Underlying A/V mixer.
    mixer: Mixer<SRC, StreamId<ID>>,
    /// Maximum number of visible participants in layouts.
    max_visibles: Option<usize>,
    /// Display names that will appear in output video
    names: HashMap<StreamId<ID>, String>,
    /// Will be used to cache a speaker whose stream was not added yet
    speaker_id: Option<(ID, SpeakerSwitchMode)>,
    /// participant who is currently speaking or `None`
    speaker: Option<ID>,
}

impl<SRC, ID> Talk<SRC, ID>
where
    SRC: Source + Debug,
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
    pub fn new(resolution: Size, sink: impl Sink, max_visibles: Option<usize>) -> Result<Self> {
        debug!("Starting a new talk...");
        trace!("new( {resolution:?}, {max_visibles:?} )");

        Ok(Self {
            mixer: Mixer::<SRC, StreamId<ID>>::new(resolution, TalkOverlay::new().into(), sink)?,
            max_visibles,
            names: HashMap::new(),
            speaker_id: None,
            speaker: None,
        })
    }

    /// Add a stream with the given ID and media type
    ///
    /// # Arguments
    ///
    /// - `id`: Identifies the stream to add
    /// - `display_name`: Human readable name which might get visible within output composite
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
        SRC: Source,
        SRC::Parameters: Debug,
    {
        trace!("add_stream( {id}, '{display_name}', {params:?}, {initial} )");

        // prepare title text overlay for the stream
        let overlay = TextOverlay::new(
            "Name Overlay",
            display_name,
            TextStyle {
                color: Color {
                    r: 0xff,
                    g: 0xff,
                    b: 0xff,
                    a: 0x80,
                },
                ..Default::default()
            },
        );

        // forward to mixer
        self.mixer.add_stream(
            id,
            display_name.to_string(),
            params,
            overlay.into(),
            initial.clone(),
        )?;

        // remember display name
        self.names.insert(id, display_name.to_string());

        // if available turn on audio but leave video off until `set_visibles()` is used
        if initial.has_audio {
            self.mixer.set_status(
                &id,
                StreamStatus {
                    has_audio: true,
                    has_video: false,
                },
            )?;
        }

        // check if the added stream was set as speaker before
        if let Some((speaker, mode)) = self.speaker_id.clone() {
            if speaker == id.id {
                // set added stream to speaker now
                self.set_speaker(Some(speaker), &mode)?;
                self.speaker_id = None;
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
        self.mixer.remove_stream(id)?;

        println!("remove a");
        if let Some(id) = self.get_first_screen_capture() {
            println!("remove a: {id:#?}");
            self.mixer.set_visible(&id.clone(), true, true);
        }

        Ok(())
    }

    /// Remove all streams from mixer.
    ///
    pub fn clear(&mut self) -> Result<()> {
        trace!("remove_all_stream()");
        let ids: Vec<StreamId<ID>> = self.mixer.streams.keys().cloned().collect();
        for id in ids {
            self.remove_stream(id).expect("cannot remove stream")
        }
        Ok(())
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

    /// Check if a given participant ID is known by the mixer.
    ///
    /// # Arguments
    ///
    /// - `id`: Describes which participant to search for.
    ///
    pub fn contains_any_stream(&self, id: &ID) -> bool {
        media_types().any(|media_type| self.contains_stream(&StreamId::new(*id, media_type)))
    }

    /// Get mutable access tp the internal stream with the given `id`.
    pub fn stream_mut(&mut self, id: &StreamId<ID>) -> Option<&mut Stream<SRC>> {
        // forward to mixer
        self.mixer.streams.get_mut(id)
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
        self.speaker_id = None;

        if let Some(speaker) = &speaker {
            // check if speaker is stream
            if !self.contains_any_stream(speaker) {
                debug!("unknown speaker is remembered to activate later");
                self.speaker_id = Some((*speaker, mode.clone()));
                return Ok(());
            }

            match mode {
                SpeakerSwitchMode::FirstShift => {
                    // remove all speaker's streams
                    while let Some(pos) = visibles.iter().position(|id| id.id == *speaker) {
                        trace!(
                            "remove {speaker} ({media_type}) from position {pos}",
                            media_type = visibles[pos].media_type
                        );
                        visibles.remove(pos);
                    }
                    // make all available media of the speaker visible
                    for media_type in media_types() {
                        let stream = StreamId::new(*speaker, media_type);
                        if self.contains_stream(&stream) {
                            let media_type_is_camera = media_type == MediaSessionType::Camera;
                            // If there is an active screen capture from someone
                            // and the updated focus is just a screen capture
                            // then the position should be the second place.
                            // Otherwise the main view would be replaced.
                            let position = if self.get_first_screen_capture().is_some()
                                && media_type_is_camera
                            {
                                1
                            } else {
                                0
                            };
                            trace!("insert new speaker {speaker} ({media_type}) at position ({position})");
                            visibles.insert(position, stream);
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
            self.mixer.set_visibles(&visibles);
        } else {
            warn!("setting no speaker currently ignored")
        }
        self.speaker = speaker;

        Ok(())
    }

    /// Get ID of current speaker or `None`
    pub fn get_speaker(&self) -> Option<ID> {
        self.speaker
    }

    /// Set status of stream with `id`.
    ///
    /// Makes video streams visible if `max_visibles` hasn't reached.
    ///
    /// # Arguments
    ///
    /// - `id`: ID of the participant's stream
    /// - `new_status`: new status for that stream
    ///
    pub fn set_status(&mut self, id: &StreamId<ID>, new_status: StreamStatus) -> Result<()> {
        info!("set_status({id}, {new_status:?}");
        self.mixer.set_status(id, new_status.clone())
    }

    /// Set title of the talk which is displayed in overlay
    ///
    /// # Arguments
    ///
    /// - `title`: title text
    ///
    pub fn set_title(&self, title: &str) {
        if let AnyOverlay::Talk(overlay) = &self.mixer.overlay {
            return overlay.set_title(title);
        }
        panic!("talk has no title overlay!")
    }

    /// Show title of the talk
    ///
    /// # Arguments
    ///
    /// - `show`: Visible if `true`
    ///
    pub fn show_title(&self, show: bool) {
        if let AnyOverlay::Talk(overlay) = &self.mixer.overlay {
            overlay.show_title(show);
        }
        panic!("talk has no title overlay!")
    }

    /// Show clock in the talk
    ///
    /// # Arguments
    ///
    /// - `show`: Visible if `true`
    ///
    pub fn show_clock(&self, show: bool) {
        if let AnyOverlay::Talk(overlay) = &self.mixer.overlay {
            overlay.show_clock(show);
        }
        panic!("talk has no clock overlay!")
    }

    /// Set title in a participant's stream
    ///
    /// # Arguments
    ///
    /// - `id`: ID of the participant's stream
    /// - `title`: title text
    ///
    pub fn set_stream_title(&self, id: &StreamId<ID>, title: &str) -> Result<()> {
        if let Some(stream) = self.mixer.streams.get(id) {
            if let AnyOverlay::Text(overlay) = &stream.overlay {
                overlay.set(title);
                return Ok(());
            }
        }
        panic!("source {id} title overlay missing")
    }

    /// Show titles in participants' streams
    ///
    /// # Arguments
    ///
    /// - `show`: Visible if `true`
    ///
    pub fn show_streams_titles(&self, show: bool) {
        for stream in self.mixer.streams.values() {
            stream.overlay.show(show);
        }
    }

    /// Try to make a stream visible.
    ///
    /// # Arguments
    ///
    /// - `id`: ID of the  stream
    ///
    /// # Return
    ///
    /// - `false` if stream has been made visible.
    /// - `true` if max visibles was exceeded and stream could not be shown.
    ///
    pub fn try_show(&mut self, stream_id: &StreamId<ID>) -> bool {
        let new_stream_is_screen_capture = stream_id.media_type == MediaSessionType::ScreenCapture;
        let noone_is_screen_capturing = self.get_first_screen_capture().is_none();
        let current_speaker_is_same_user =
            self.speaker_id == Some((stream_id.id, SpeakerSwitchMode::FirstShift));
        // Check if the new stream is a screen capture
        // If it's a screen capture and noone else is streaming, push it to the first position
        // If someone is streaming, but the current speaker is the same user, push it to the first position
        let position_first = new_stream_is_screen_capture
            && (noone_is_screen_capturing || current_speaker_is_same_user);
        self.mixer.set_visible(stream_id, position_first, true)
    }

    /// Return `true`, if a participant's stream is currently visible
    ///
    /// # Arguments
    ///
    /// - `id`: ID of the participant's stream
    ///
    pub fn is_any_visible(&self, id: &ID) -> bool {
        media_types().any(|media_type| self.mixer.is_visible(&StreamId::new(*id, media_type)))
    }

    /// Apply given layout `L` to mixer.
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
            .map(|stream| &mut stream.source)
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

    fn get_first_screen_capture(&self) -> Option<StreamId<ID>> {
        self.mixer
            .visibles
            .clone()
            .into_iter()
            .filter(|visible| visible.media_type == MediaSessionType::ScreenCapture)
            .collect::<Vec<_>>()
            .first()
            .cloned()
    }
}

impl<SRC, ID> Drop for Talk<SRC, ID>
where
    SRC: Source + Debug,
    SRC::Parameters: Debug,
    ID: Eq + Ord + Hash + Copy + Display + Debug + Sync + Send,
{
    fn drop(&mut self) {
        debug!("Stopped Talk");
        // remove all streams
        self.clear().unwrap();
    }
}

/// Mode in which the current speaker shall be displayed.
#[derive(Debug, Clone, PartialEq, Eq)]
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

impl Default for SpeakerSwitchMode {
    fn default() -> Self {
        Self::FirstShift
    }
}
