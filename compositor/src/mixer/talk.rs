// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

//! Talk manages a conference recording.

use anyhow::{bail, Context, Result};
use core::{
    fmt::{Debug, Display},
    hash::Hash,
};
use gst::Pipeline;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{
    debug, AnyOverlay, Font, Layout, Mixer, Overlay, Sink, Size, Source, Stream, StreamStatus,
    TalkOverlay, TextOverlay, TextStyle,
};

const NAME_FONT_SIZE: u32 = 16;

/// return available media types
#[must_use]
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

/// Stream ID consisting of one stream ID and a stream type.
///
/// # Types
///
/// - `ID`: Type which can identify a stream.
///
#[derive(Debug, Hash, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct StreamId<ID>
where
    ID: Eq + Ord + Hash + Copy + Debug + Display,
{
    /// ID identifying the stream
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
    /// - `id`: ID of the stream
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
    /// - `id`: ID of the stream
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
    /// - `id`: ID of the stream
    /// - `stream`: type of the stream
    ///
    pub fn new(id: ID, stream: MediaSessionType) -> Self {
        Self {
            id,
            media_type: stream,
        }
    }
}

/// A talk consisting of streams and managing maximum amount of visibles.
///
/// # Types
///
/// - `SRC`: Source type which will be created when adding a stream.
/// - `ID`: Type which can identify a stream.
///
#[derive(Debug)]
pub struct Talk<SRC, ID>
where
    SRC: Source,
    ID: Eq + Ord + Hash + Copy + Display + Debug + Sync + Send,
{
    /// Underlying A/V mixer.
    mixer: Mixer<SRC, StreamId<ID>>,
    /// Maximum number of visible streams in layouts.
    max_visibles: usize,
    /// Display names that will appear in output video
    names: HashMap<StreamId<ID>, String>,
    /// stream who is currently speaking or `None`
    current_speaker: Option<ID>,
}

impl<SRC, ID> Talk<SRC, ID>
where
    SRC: Source,
    ID: Eq + Ord + Hash + Copy + Display + Debug + Sync + Send,
{
    /// Create new Talk which creates an own Pipeline.
    ///
    /// # Arguments
    ///
    /// - `resolution`: Output video resolution.
    /// - `sink_params`: Parameters to create the output sink.
    /// - `max_visibles`: Maximum number of currently visible streams.
    ///
    /// # Errors
    ///
    /// This can fail if the `Mixer` can't be initialized.
    pub fn new(
        resolution: Size,
        layout: impl Layout,
        max_visibles: usize,
        video_support: bool,
    ) -> Result<Self> {
        Self::new_with_pipeline(
            Pipeline::new(Some("Compositor")),
            resolution,
            layout,
            max_visibles,
            video_support,
        )
    }

    /// Create new Talk for the given Pipeline.
    ///
    /// # Arguments
    ///
    /// - `pipeline`: The base pipeline which should be used to add the mixer.
    /// - `resolution`: Output video resolution.
    /// - `sink_params`: Parameters to create the output sink.
    /// - `max_visibles`: Maximum number of currently visible streams.
    ///
    /// # Errors
    ///
    /// This can fail if the `Mixer` can't be initialized.
    pub fn new_with_pipeline(
        pipeline: Pipeline,
        resolution: Size,
        layout: impl Layout,
        max_visibles: usize,
        video_support: bool,
    ) -> Result<Self> {
        debug!("Starting a new talk...");
        trace!("new( {resolution:?}, {max_visibles:?} )");

        let mixer = Mixer::<SRC, StreamId<ID>>::create(
            pipeline,
            resolution,
            layout,
            TalkOverlay::create()
                .context("unable to create TalkOverlay")?
                .into(),
            video_support,
        )
        .context("unable to create mixer")?;

        Ok(Self {
            mixer,
            max_visibles,
            names: HashMap::new(),
            current_speaker: None,
        })
    }

    /// Link the given sink to the mixer.
    ///
    /// # Errors
    ///
    /// This can fail if the mixer was unable to link the sink to the mixer.
    pub fn link_sink(&mut self, name: &str, sink: impl Sink) -> Result<()> {
        self.mixer.link_sink(name, sink)
    }

    /// Link the given sink to the mixer.
    ///
    /// # Errors
    ///
    /// This can fail if the mixer was unable to link the sink to the mixer.
    pub fn release_sink(&mut self, name: &String) -> Result<()> {
        self.mixer.release_sink(name)
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
    /// # Errors
    ///
    /// This can fail if the status of the stream can't be set.
    pub fn add_stream(
        &mut self,
        id: StreamId<ID>,
        display_name: &str,
        params: SRC::Parameters,
        initial: StreamStatus,
    ) -> Result<()>
    where
        SRC: Source,
    {
        trace!("add_stream( {id}, '{display_name}', {params:?}, {initial} )");

        // prepare title text overlay for the stream
        let overlay = TextOverlay::create(
            "Name Overlay",
            display_name,
            TextStyle {
                font: Font {
                    size: NAME_FONT_SIZE,
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .context("unable to create TextOverlay")?;

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
        self.mixer.set_status(&id, initial)?;

        Ok(())
    }

    /// Remove a stream by stream ID.
    ///
    /// # Arguments
    ///
    /// - `id`: Describes which stream shall be removed.
    ///
    /// # Errors
    ///
    /// This can fail if the stream can't be removed from the `Mixer`.
    pub fn remove_stream(&mut self, id: StreamId<ID>) -> Result<()> {
        trace!("remove_stream( {id} )");

        // remove name
        self.names.remove(&id);
        // forward to mixer
        self.mixer.remove_stream(id)?;

        // After removing push the next screen share in the list to the first
        // position
        if let Some(stream_id) = self.get_first_screen_capture() {
            self.mixer
                .set_stream_to_first_position(&stream_id)
                .context("unable to set stream with id '{stream_id}' to first position")?;
        }

        Ok(())
    }

    /// Remove all streams from mixer.
    ///
    /// # Errors
    ///
    /// This can fail if some stream cannot be removed.
    pub fn clear(&mut self) -> Result<()> {
        trace!("remove_all_stream()");
        let ids: Vec<StreamId<ID>> = self.mixer.streams.keys().copied().collect();
        for id in ids {
            self.remove_stream(id)
                .with_context(|| format!("cannot remove stream {id}"))?;
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

    /// Check if a given stream ID is known by the mixer.
    ///
    /// # Arguments
    ///
    /// - `id`: Describes which stream to search for.
    ///
    pub fn contains_any_stream(&self, id: &ID) -> bool {
        media_types().any(|media_type| self.contains_stream(&StreamId::new(*id, media_type)))
    }

    /// Get mutable access tp the internal stream with the given `id`.
    pub fn stream_mut(&mut self, id: &StreamId<ID>) -> Option<&mut Stream<SRC>> {
        // forward to mixer
        self.mixer.streams.get_mut(id)
    }

    /// Set which stream will be visualized as speaker.
    ///
    /// # Arguments
    ///
    /// - `speaker`: Stream of the speaker or `None`.
    /// - `mode`: How the speaker comes into the scene.
    ///
    /// # Errors
    ///
    /// This can fail if the speaker cannot be set to the first or second position.
    pub fn set_speaker(&mut self, speaker: ID) -> Result<()> {
        info!("set_speaker( {speaker:?} )");

        self.current_speaker = Some(speaker);

        let stream_id = StreamId::new(speaker, MediaSessionType::ScreenCapture);
        if let Some(stream) = self.mixer.streams.get(&stream_id) {
            // The speaker has no screen, so it doesn't need to update the position
            if stream.status.has_video {
                self.mixer
                    .set_stream_to_first_position(&stream_id)
                    .context("unable to set stream with id '{stream_id}' to first position")?;
            }
        }

        let stream_id = StreamId::new(speaker, MediaSessionType::Camera);
        if let Some(stream) = self.mixer.streams.get(&stream_id) {
            // The speaker has no screen, so it doesn't need to update the position
            if stream.status.has_video {
                // check if noone is sharing their screen or the new speaker is also screen sharing
                if self.get_first_screen_capture().is_none() {
                    self.mixer
                        .set_stream_to_first_position(&stream_id)
                        .context("unable to set stream with id '{stream_id}' to first position")?;
                } else {
                    self.mixer
                        .set_stream_to_second_position(&stream_id)
                        .context("unable to set stream with id '{stream_id}' to second position")?;
                }
            }
        }

        Ok(())
    }

    pub fn unset_speaker(&mut self) {
        self.current_speaker = None;
    }

    /// Get ID of current speaker or `None`
    pub fn get_current_speaker(&self) -> Option<ID> {
        self.current_speaker
    }

    /// Set status of stream with `id`.
    ///
    /// Makes video streams visible if `max_visibles` hasn't reached.
    ///
    /// # Arguments
    ///
    /// - `id`: ID of the stream
    /// - `new_status`: new status for that stream
    ///
    /// # Errors
    ///
    /// This can fail if the status of the stream can't be set in the `Mixer`.
    pub fn set_status(&mut self, id: &StreamId<ID>, new_status: &StreamStatus) -> Result<()> {
        info!("set_status({id}, {new_status:?}");
        let Some(current_stream) = self.mixer.streams.get(id) else {
            debug!("current_stream not found for id: {id:?}");
            return Ok(());
        };
        let old_status = current_stream.status.clone();

        self.mixer.set_status(id, new_status.clone())?;

        match (old_status.has_video, new_status.has_video) {
            (false, true) => self
                .show_stream(id)
                .context("unable to show stream for id '{id}'")?,
            (true, false) => self
                .hide_stream(id)
                .context("unable to hide stream for id '{id}'")?,
            _ => {}
        }

        Ok(())
    }

    /// Set title of the talk which is displayed in overlay
    ///
    /// # Arguments
    ///
    /// - `title`: title text
    ///
    /// # Errors
    ///
    /// This can fail if the `Talk` has no `AnyOverlay::Talk`
    pub fn set_title(&self, title: &str) -> Result<()> {
        if let AnyOverlay::Talk(overlay) = &self.mixer.overlay {
            overlay.set_title(title);
            return Ok(());
        }
        bail!("talk has no title overlay!")
    }

    /// Show title of the talk
    ///
    /// # Arguments
    ///
    /// - `show`: Visible if `true`
    ///
    /// # Errors
    ///
    /// This can fail if the `Talk` has no `AnyOverlay::Talk`
    pub fn show_title(&self, show: bool) -> Result<()> {
        if let AnyOverlay::Talk(overlay) = &self.mixer.overlay {
            overlay.show_title(show);
            return Ok(());
        }
        bail!("talk has no title overlay!")
    }

    /// Show clock in the talk
    ///
    /// # Arguments
    ///
    /// - `show`: Visible if `true`
    ///
    /// # Errors
    ///
    /// This can fail if the `Talk` has no `AnyOverlay::Talk`
    pub fn show_clock(&self, show: bool) -> Result<()> {
        if let AnyOverlay::Talk(overlay) = &self.mixer.overlay {
            overlay.show_clock(show);
            return Ok(());
        }
        bail!("talk has no clock overlay!")
    }

    /// Set title in a stream
    ///
    /// # Arguments
    ///
    /// - `id`: ID of the stream
    /// - `title`: title text
    ///
    /// # Errors
    ///
    /// This can fail if the `Talk` has no `AnyOverlay::Talk`
    pub fn set_stream_title(&self, id: &StreamId<ID>, title: &str) -> Result<()> {
        if let Some(stream) = self.mixer.streams.get(id) {
            if let AnyOverlay::Text(overlay) = &stream.overlay {
                overlay.set(title);
                return Ok(());
            }
        }
        bail!("source {id} title overlay missing")
    }

    /// Show titles in streams
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
    /// # Errors
    ///
    /// This can fail if the `Mixer` cannot hide an old stream or show the new stream.
    pub fn show_stream(&mut self, stream_id: &StreamId<ID>) -> Result<()> {
        // Check if the maximum amount of streams is reached
        if self.mixer.visibles.len() >= self.max_visibles {
            // If the new stream is just a camera feed, then don't show them
            if stream_id.media_type == MediaSessionType::Camera {
                return Ok(());
            }
            // The new camera feed is a screen share, which has a higher
            // priority, so the latest stream will be removed
            if let Some(id) = self.mixer.visibles.last().copied() {
                self.mixer
                    .hide_stream(&id)
                    .context("unable hide stream for id '{id}'")?;
            }
        }
        // Check if the new stream is a screen capture
        // If it's a screen capture and noone else is streaming, push it to the first position
        // If someone is streaming, but the current speaker is the same user, push it to the first position
        let position_first = stream_id.media_type == MediaSessionType::ScreenCapture
            && self.get_first_screen_capture().is_none();

        self.mixer.show_stream(stream_id, position_first)
    }

    /// Try to hide the stream.
    ///
    /// # Errors
    ///
    /// This can fail if the `Mixer` cannot hide the given stream.
    pub fn hide_stream(&mut self, stream_id: &StreamId<ID>) -> Result<()> {
        self.mixer
            .hide_stream(stream_id)
            .context("unable to hide_stream in mixer")
    }

    /// Return `true`, if a stream is currently visible
    ///
    /// # Arguments
    ///
    /// - `id`: ID of the stream
    ///
    pub fn is_any_visible(&self, id: &ID) -> bool {
        media_types().any(|media_type| self.mixer.is_visible(&StreamId::new(*id, media_type)))
    }

    /// Change the current layout
    ///
    /// # Errors
    ///
    /// This can fail if the `Mixer` cannot rerender the new layout.
    pub fn change_layout(&mut self, layout: impl Layout) -> Result<()> {
        self.mixer
            .change_layout(layout)
            .context("unable to change_layout in mixer")
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
        self.mixer.dot(filename_without_extension, params);
    }

    fn get_first_screen_capture(&self) -> Option<StreamId<ID>> {
        self.mixer
            .visibles
            .clone()
            .into_iter()
            .find(|visible| visible.media_type == MediaSessionType::ScreenCapture)
    }
}

impl<SRC, ID> Drop for Talk<SRC, ID>
where
    SRC: Source,
    ID: Eq + Ord + Hash + Copy + Display + Debug + Sync + Send,
{
    fn drop(&mut self) {
        debug!("Stopped Talk");
        // remove all streams
        if let Err(error) = self.clear() {
            error!("unable to clear the Talk, error: {error}");
        }
    }
}
