// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use std::{
    collections::{HashMap, HashSet},
    fmt::{Debug, Display},
    hash::Hash,
    sync::atomic::{AtomicU64, Ordering},
};

use anyhow::{anyhow, bail, Context, Result};
use gst::{
    event::Reconfigure, prelude::*, Bin, Clock, ClockTime, Element, ElementFactory, Fraction,
    GhostPad, MessageView, Pipeline, SystemClock,
};
use types::{
    core::ParticipantId,
    signaling::media::{self, MediaSessionState, MediaSessionType},
};

mod audio_mixer;
mod bus;
pub mod debug;
mod sink;
mod source;
mod stream;
mod text_style;
mod video_mixer;

use self::{audio_mixer::AudioMixer, sink::ActiveSink, video_mixer::VideoMixer};
use crate::{
    elements::register_all, mixer::bus::PipelineWatched, GstBinErrorExt, GstElementBuilderErrorExt,
    GstElementErrorExt, GstGhostPadErrorExt, GstPadErrorExt,
};

#[rustfmt::skip]
pub use {
    sink::*,
    source::*,
    stream::*,
    text_style::*,

    super::{layout::*, overlays::*}
};

pub(crate) const AUDIO_SAMPLE_RATE: i32 = 48_000;
pub(crate) const AUDIO_CHANNELS: i32 = 2;

pub(crate) const NAME_FONT_SIZE: u32 = 16;

pub const AUDIO_INTER_COMPOSITOR: &str = "audio-compositor";
pub const VIDEO_INTER_COMPOSITOR: &str = "video-compositor";

pub static INTER_COUNTER: AtomicU64 = AtomicU64::new(0u64);

/// `MediaDescriptor` identifies a media stream by participant and media type.
#[derive(Debug, Hash, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MediaDescriptor {
    pub participant_id: ParticipantId,
    pub media_type: MediaSessionType,
}

impl From<media::event::Source> for MediaDescriptor {
    fn from(value: media::event::Source) -> Self {
        Self {
            participant_id: value.source,
            media_type: value.media_session_type,
        }
    }
}

impl From<MediaDescriptor> for media::command::Target {
    fn from(value: MediaDescriptor) -> Self {
        Self {
            target: value.participant_id,
            media_session_type: value.media_type,
        }
    }
}

impl Display for MediaDescriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "#{participant_id:?}/{stream}",
            participant_id = self.participant_id,
            stream = self.media_type
        )
    }
}

/// Mixer managing the `GStreamer` pipeline using the given layout and source type
///
/// Here is an example pipeline:
/// <div>
/// <img src="../../../compositor/images/1_add_streams.png" width="1000" />
/// </div>
///
/// # Types
///
/// - `SRC`: Source type to use when adding streams.
/// - `SINK`: Sink type to create output
/// - `ID`: stream identifier type
///
#[derive(Debug)]
pub struct Mixer<SRC>
where
    SRC: Source,
{
    streams: HashMap<MediaDescriptor, Stream<SRC>>,
    producer_id: u64,
    visibles: Vec<MediaDescriptor>,
    audio_mixer: AudioMixer,
    video_mixer: Option<VideoMixer>,
    pipeline: PipelineWatched,
    overlay: TalkOverlay,
    sinks: HashMap<String, ActiveSink>,
    output_resolution: Size,
    layout: Box<dyn Layout>,
    system_clock: Clock,
    max_visibles: usize,
}

impl<SRC> Mixer<SRC>
where
    SRC: Source,
{
    /// Create a new mixer and setup the initial `GStreamer` pipeline with the given type of sink.
    /// If the pipeline is `None` we create a new one.
    ///
    /// # Arguments
    /// - `existing_pipeline`: Provided pipeline to use and extend.
    /// - `output_resolution`: Output video resolution.
    /// - `layout`: The layout which will be used.
    /// - `max_visibles`: Maximum number of visible streams
    /// - `video_support`: flag to confiure video support.
    ///
    /// # Errors
    ///
    /// This can fail if adding the pipeline and elements in `GStreamer` isn't working.
    pub fn create(
        output_resolution: Size,
        layout: impl Layout,
        max_visibles: usize,
        video_support: bool,
        clock_format: &ClockFormat,
    ) -> Result<Self> {
        register_all().context("Unable to register all custom GStreamer Elements")?;

        let pipeline = PipelineWatched::new("Compositor", true, false)
            .context("unable to create Compositor PipelineWatched")?;

        debug!("create compositor ( {output_resolution:?}, {max_visibles:?} )");
        let producer_id = INTER_COUNTER.fetch_add(1, Ordering::Relaxed);

        let overlay = TalkOverlay::create(clock_format).context("unable to create TalkOverlay")?;
        let audio_mixer = AudioMixer::create(producer_id).context("unable to create AudioMixer")?;
        pipeline.add_with_context(audio_mixer.bin())?;

        let video_mixer = if video_support {
            let video_mixer = VideoMixer::create(output_resolution, &overlay, producer_id)
                .context("unable to create VideoMixer")?;

            pipeline.add_with_context(video_mixer.bin())?;

            Some(video_mixer)
        } else {
            None
        };

        let system_clock = SystemClock::obtain();
        pipeline.use_clock(Some(&system_clock));
        pipeline.set_base_time(ClockTime::ZERO);
        pipeline.set_start_time(None);

        pipeline.set_state_with_context(gst::State::Playing)?;

        let sinks = HashMap::<String, ActiveSink>::new();

        let mixer = Mixer {
            audio_mixer,
            video_mixer,
            visibles: Vec::new(),
            pipeline,
            streams: HashMap::new(),
            overlay,
            sinks,
            output_resolution,
            layout: Box::new(layout),
            system_clock,
            max_visibles,
            producer_id,
        };

        Ok(mixer)
    }

    /// Enable video support for the current compositor.
    ///
    /// # Errors
    ///
    /// This can fail if the `VideoMixer` couldn't be created or the `ActiveSink`
    /// failed to link to the `VideoMixer`.
    pub fn enable_video(&mut self) -> Result<()> {
        if self.video_mixer.is_some() {
            return Ok(());
        }

        let video_mixer =
            VideoMixer::create(self.output_resolution, &self.overlay, self.producer_id)
                .context("unable to create VideoMixer")?;

        self.pipeline.add_with_context(video_mixer.bin())?;

        video_mixer
            .bin()
            .set_state_with_context(gst::State::Playing)?;

        for key in self.streams.keys().copied().collect::<Vec<_>>() {
            self.enable_video_support_if_possible(&key)?;
        }

        for sink in self.sinks.values() {
            sink.link_video_mixer(self.producer_id)
                .context("unable to link VideoMixer to sink")?;
            sink.pipeline.set_state_with_context(gst::State::Playing)?;
        }

        self.video_mixer = Some(video_mixer);

        Ok(())
    }

    /// Add a new stream to the mixer.
    ///
    /// New video streams will NOT get visible but audio streams will
    /// be hearable.
    ///
    /// # Arguments
    ///
    /// - `id`: Unique identifier of the stream.
    /// - `display_name`: Name to display to user as identifier.
    /// - `params`: Source specific parameters.
    /// - `overlays`: list of overlays to attach behind source
    ///
    /// # Errors
    ///
    /// This can fail if adding the stream to the `GStreamer` pipeline fails.
    pub fn add_stream(
        &mut self,
        descriptor: MediaDescriptor,
        display_name: String,
        params: SRC::Parameters,
        initial: MediaSessionState,
    ) -> Result<()>
    where
        SRC: Source,
    {
        trace!("add_stream( {descriptor}, '{display_name}', {params:?}, {initial} )");

        // prepare title text overlay for the stream
        let overlay = TextOverlay::create(
            "Name Overlay",
            display_name.as_str(),
            TextStyle {
                font: Font {
                    size: NAME_FONT_SIZE,
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .context("unable to create TextOverlay")?;

        // check if stream ID is already known
        if self.streams.contains_key(&descriptor) {
            warn!("Cannot add stream with ID {descriptor} twice.");
            return Err(anyhow!("Cannot add stream with ID {descriptor} twice."));
        }

        // create new source bin
        let source = SRC::create(&descriptor, params).context("unable to create Source")?;
        let bin = Bin::builder()
            .name(format!("Overlay: {descriptor}"))
            .build();

        // Add source bin to bin
        bin.add_with_context(&source.bin())?;

        // Add bin to pipeline
        self.pipeline.add_with_context(&bin)?;

        // Link audio in pipeline
        let audio_ghost_pad = GhostPad::with_target_with_context(None, &source.audio())?;
        bin.add_pad_with_context(&audio_ghost_pad)?;
        let audio = self
            .audio_mixer
            .link_src(&audio_ghost_pad)
            .context("unable to add 'audio' pad to 'audio_mixer'")?;

        // Link video in pipeline
        let video = if let (Some(video_mixer), Some(video)) = (&self.video_mixer, source.video()) {
            let overlay_src_pad = overlay
                .src()
                .context("unable to get src pad from overlay")?;
            let overlay_ghost_pad = GhostPad::with_target_with_context(None, &overlay_src_pad)?;

            bin.add_pad_with_context(&overlay_ghost_pad)?;

            video_mixer
                .link_src(&overlay_ghost_pad)
                .context("unable to add 'video' pad to 'video_mixer'")?;

            Some(video)
        } else {
            None
        };

        debug::debug_dot(&*self.pipeline, "stream_added");

        bin.sync_state_with_parent_with_context()?;

        self.streams.insert(
            descriptor,
            Stream {
                display_name,
                source,
                bin,
                video,
                audio,
                overlay,
                status: initial,
            },
        );

        // Setup video in pipeline
        self.enable_video_support_if_possible(&descriptor)?;

        debug!("Added stream {descriptor}");

        // if available turn on audio but leave video off until `set_visibles()` is used
        self.set_stream_status(descriptor, initial)?;

        Ok(())
    }

    pub(crate) fn enable_video_support_if_possible(
        &mut self,
        media_descriptor: &MediaDescriptor,
    ) -> Result<()> {
        let Some(video_mixer) = &self.video_mixer else {
            return Ok(());
        };
        let Some(source) = self.streams.get_mut(media_descriptor) else {
            return Ok(());
        };
        let Some(video) = &source.video else {
            return Ok(());
        };

        let videoconvertscale = ElementFactory::make("videoconvertscale")
            .name("videoconvertscale")
            .build_with_context()?;
        let capsfilter = ElementFactory::make("capsfilter")
            .name("capsfilter")
            .build_with_context()?;

        source.bin.add_many_with_context(&[
            &videoconvertscale,
            &capsfilter,
            source.overlay.element(),
        ])?;

        Element::link_many_with_context(&[
            &videoconvertscale,
            &capsfilter,
            source.overlay.element(),
        ])?;

        let videoconvertscale_sink_pad = videoconvertscale.static_pad_with_context("sink")?;
        video.link_with_context(&videoconvertscale_sink_pad)?;

        let overlay_src_pad = source
            .overlay
            .src()
            .context("unable to get src pad from overlay")?;
        let overlay_ghost_pad =
            GhostPad::with_target_with_context(Some("overlay-src"), &overlay_src_pad)?;

        source.bin.add_pad_with_context(&overlay_ghost_pad)?;

        let video = video_mixer
            .link_src(&overlay_ghost_pad)
            .context("unable to add 'video' pad to 'video_mixer'")?;

        source.video = Some(video);

        videoconvertscale.set_state(gst::State::Playing)?;
        capsfilter.set_state(gst::State::Playing)?;
        source.overlay.element().set_state(gst::State::Playing)?;

        Ok(())
    }

    /// remove a stream from the mixer.
    ///
    /// # Arguments
    ///
    /// - `descriptor`: Identifier of the stream.
    ///
    /// # Errors
    ///
    /// This can fail if the stream bin can't be set to NULL.
    pub fn remove_stream(&mut self, descriptor: MediaDescriptor) -> Result<()> {
        trace!("remove_stream( {descriptor} )");

        // remove stream from stored streams
        let stream = self
            .streams
            .remove(&descriptor)
            .ok_or_else(|| anyhow!("given stream id ({descriptor}) cannot be found"))?;

        // remove bin from pipeline
        stream.bin.set_state_with_context(gst::State::Null)?;

        trace!("releasing requested pads from mixers");

        if let Some(sink) = stream.audiomixer_sink() {
            self.audio_mixer
                .release_src(&sink)
                .context("unable to release src in audio_mixer")?;
        }

        if let Some(video_mixer) = &self.video_mixer {
            if let Some(video_src) = &stream.compositor_sink() {
                video_mixer
                    .release_src(video_src)
                    .context("unable to release src in video_mixer")?;
            }
        }

        self.pipeline
            .remove(&stream.bin)
            .context("can not remove stream's bin from pipeline")?;

        // remove stream from visibles
        if let Some(index) = self.visibles.iter().position(|i| *i == descriptor) {
            self.visibles.remove(index);
            self.rerender_layout()
                .context("unable to rerender layout")?;
        }

        // After removing push the next screen share in the list to the first
        // position
        if let Some(descriptor) = self.get_first_screen_capture() {
            self.set_stream_to_active_position(descriptor)
                .context("unable to set stream with id '{descriptor}' to first position")?;
        }

        Ok(())
    }

    /// Check if a given stream ID is known by the mixer.
    ///
    /// # Arguments
    ///
    /// - `descriptor`: Stream identifier.
    ///
    #[must_use]
    pub fn contains_stream(&self, descriptor: MediaDescriptor) -> bool {
        // forward to mixer
        self.streams.contains_key(&descriptor)
    }

    /// Get mutable access tp the internal stream with the given `id`.
    #[must_use]
    pub fn stream_mut(&mut self, descriptor: MediaDescriptor) -> Option<&mut Stream<SRC>> {
        // forward to mixer
        self.streams.get_mut(&descriptor)
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
    pub fn set_speaker(&mut self, speaker: ParticipantId) -> Result<()> {
        info!("set_speaker( {speaker:?} )");

        let descriptor = MediaDescriptor {
            participant_id: speaker,
            media_type: MediaSessionType::Screen,
        };
        if let Some(stream) = self.streams.get(&descriptor) {
            // The speaker has no screen, so it doesn't need to update the position
            if stream.status.video {
                self.set_stream_to_active_position(descriptor)
                    .context("unable to set stream with id '{descriptor}' to first position")?;
            }
        }

        let descriptor = MediaDescriptor {
            participant_id: speaker,
            media_type: MediaSessionType::Video,
        };
        if let Some(stream) = self.streams.get(&descriptor) {
            // The speaker has no screen, so it doesn't need to update the position
            if stream.status.video {
                // check if noone is sharing their screen or the new speaker is also screen sharing
                if self.get_first_screen_capture().is_none() {
                    self.set_stream_to_active_position(descriptor)
                        .context("unable to set stream with id '{descriptor}' to first position")?;
                } else {
                    self.set_stream_to_position(descriptor, 1).context(
                        "unable to set stream with id '{descriptor}' to second position",
                    )?;
                }
            }
        }

        Ok(())
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
    pub fn set_status(
        &mut self,
        descriptor: MediaDescriptor,
        new_status: MediaSessionState,
    ) -> Result<()> {
        info!("set_status({descriptor}, {new_status:?}");
        let Some(current_stream) = self.streams.get(&descriptor) else {
            debug!("current_stream not found for descriptor: {descriptor:?}");
            return Ok(());
        };
        let old_status = current_stream.status;

        self.set_stream_status(descriptor, new_status)?;

        match (old_status.video, new_status.video) {
            (false, true) => self
                .show_stream(descriptor)
                .context("unable to show stream for descriptor '{descriptor}'")?,
            (true, false) => self
                .hide_stream(descriptor)
                .context("unable to hide stream for descriptor '{descriptor}'")?,
            _ => {}
        }

        Ok(())
    }

    /// Set title of the talk which is displayed in overlay
    ///
    /// # Arguments
    ///
    /// - `title`: title text
    pub fn set_title(&self, title: &str) {
        self.overlay.set_title(title);
    }

    /// Set title overlay for a stream
    ///
    /// # Arguments
    ///
    /// - `descriptor`: Stream identifier
    /// - `title`: title text, e.g. participant name
    ///
    /// # Errors
    ///
    /// Fails if the requested stream is missing
    pub fn set_stream_title(&self, descriptor: MediaDescriptor, title: &str) -> Result<()> {
        let stream = self
            .streams
            .get(&descriptor)
            .with_context(|| format!("set_title failed. Stream {descriptor} not found"))?;
        stream.overlay.set(title);
        Ok(())
    }

    /// Set title overlay visibility for all streams
    ///
    /// # Arguments
    ///
    /// - `show`: overlay visibility flag
    ///
    pub fn show_streams_titles(&self, show: bool) {
        for stream in self.streams.values() {
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
    pub fn show_stream(&mut self, descriptor: MediaDescriptor) -> Result<()> {
        // Check if the maximum amount of streams is reached
        if self.visibles.len() >= self.max_visibles {
            // If the new stream is just a camera feed, then don't show them
            if descriptor.media_type == MediaSessionType::Video {
                return Ok(());
            }
            // The new camera feed is a screen share, which has a higher
            // priority, so the latest stream will be removed
            if let Some(descriptor) = self.visibles.last().copied() {
                self.hide_stream(descriptor)
                    .context("unable hide stream for id '{id}'")?;
            }
        }
        // Check if the new stream is a screen capture
        // If it's a screen capture and noone else is streaming, push it to the first position
        // If someone is streaming, but the current speaker is the same user, push it to the first position
        let position_first = descriptor.media_type == MediaSessionType::Screen
            && self.get_first_screen_capture().is_none();

        if self.is_visible(descriptor) {
            return Ok(());
        }

        if position_first {
            self.visibles.insert(0, descriptor);
        } else {
            self.visibles.push(descriptor);
        }
        self.rerender_layout().context("unable to rerender layout")
    }

    /// Get mutable access to a source specified by stream ID.
    ///
    /// # Arguments
    ///
    /// - `descriptor`: Stream identifier
    ///
    pub fn get_source(&mut self, descriptor: MediaDescriptor) -> Option<&mut SRC> {
        self.streams
            .get_mut(&descriptor)
            .map(|stream| &mut stream.source)
    }

    fn get_first_screen_capture(&self) -> Option<MediaDescriptor> {
        self.visibles
            .clone()
            .into_iter()
            .find(|visible| visible.media_type == MediaSessionType::Screen)
    }

    /// Link the given sink to the mixer.
    ///
    /// # Errors
    ///
    /// This can fail if the audio or video sink could not be linked to the mixer.
    pub fn link_sink(&mut self, name: &str, sink: impl Sink) -> Result<()> {
        trace!("link sink, name: {name}, sinke: {sink:?}");
        if self.sinks.contains_key(name) {
            bail!("a stream with the name '{name}' already exists");
        }

        let pipeline = PipelineWatched::new(name, sink.init_bus_watch(), sink.requires_eos())
            .context("unable to create PipelineWatched")?;

        pipeline.use_clock(Some(&self.system_clock));
        pipeline.set_base_time(ClockTime::ZERO);
        pipeline.set_start_time(None);

        let bin = sink.bin();
        pipeline.add_with_context(&bin)?;

        let mut active_sink = ActiveSink {
            pipeline,
            inner: Box::new(sink),
        };

        active_sink
            .link_audio_mixer(self.producer_id)
            .context("unable to link AudioMixer to sink")?;

        if self.video_mixer.is_some() {
            active_sink
                .link_video_mixer(self.producer_id)
                .context("unable to link VideoMixer to sink")?;
        }

        active_sink
            .pipeline
            .set_state_with_context(gst::State::Playing)?;
        active_sink
            .pipeline
            .sync_children_states()
            .context("unable to sync children states for pipeline")?;

        active_sink
            .inner
            .on_play()
            .context("unable to set sink to playing")?;

        debug::dot(&*self.pipeline, "link-sink-main-pipeline");
        debug::dot(
            &*active_sink.pipeline,
            format!("link-sink_sink-pipeline_{name}").as_str(),
        );

        self.sinks.insert(name.to_owned(), active_sink);

        Ok(())
    }

    /// Release the given sink from the mixer.
    ///
    /// # Errors
    ///
    /// This can fail if the sink could not be released from the mixer.
    pub async fn release_sink(&mut self, name: &String) {
        trace!("release_sink {name}");
        if let Some(mut sink) = self.sinks.remove(name) {
            sink.pipeline.drop().await;
        }
    }

    /// Set stream to the first position
    ///
    /// # Arguments
    ///
    /// `descriptor`: Stream identifier
    ///
    /// # Errors
    ///
    /// This can fail if the `set_stream_to_position` function is failing.
    fn set_stream_to_active_position(&mut self, descriptor: MediaDescriptor) -> Result<()> {
        // Do not rerender the position if the same two participants
        // are already in the room. This would cause an uneccessary
        // flickering effect (swapping the two speaker).
        if self.visibles.contains(&descriptor) && self.visibles.len() <= 2 {
            return Ok(());
        }

        self.set_stream_to_position(descriptor, 0)
    }

    /// Set a stream to a new position
    ///
    /// # Arguments
    ///
    /// `descriptor`: Stream identifier
    /// `position`: The position to set the stream to. Values greater than the currently visible streams will be clamped.
    ///
    /// # Errors
    ///
    /// This can fail if the `rerender_layout` function is failing.
    fn set_stream_to_position(
        &mut self,
        descriptor: MediaDescriptor,
        position: usize,
    ) -> Result<()> {
        // Clamp the position to avoid out-of-bounds in Vec::insert
        let position = position.min(self.visibles.len());

        if self.visibles.get(position) == Some(&descriptor) {
            return Ok(());
        }

        self.visibles
            .retain(|other_descriptor| other_descriptor != &descriptor);
        self.visibles.insert(position, descriptor);
        self.rerender_layout().context("unable to rerender layout")
    }

    /// Hide stream.
    ///
    /// # Arguments
    ///
    /// `descriptor`: Stream identifier
    ///
    /// # Errors
    ///
    /// This can fail if the `rerender_layout` function is failing.
    fn hide_stream(&mut self, descriptor: MediaDescriptor) -> Result<()> {
        if !self.is_visible(descriptor) {
            return Ok(());
        }

        self.visibles
            .retain(|other_descriptor| other_descriptor != &descriptor);
        self.rerender_layout().context("unable to rerender layout")
    }

    /// Return `true`, if stream is currently visible
    fn is_visible(&self, descriptor: MediaDescriptor) -> bool {
        self.visibles.contains(&descriptor)
    }

    /// Set status of a stream.
    ///
    /// This function does not change visibility of a stream but audio presence.
    ///
    /// # Arguments
    ///
    /// - `descriptor`: Identifier of the stream that shall be updated.
    /// - `new_status`: New status to override.
    ///
    /// # Errors
    ///
    /// This can fail if the stream isn't in the `streams` list.
    fn set_stream_status(
        &mut self,
        descriptor: MediaDescriptor,
        new_status: MediaSessionState,
    ) -> Result<()> {
        info!("set_status( {descriptor}, {new_status} )");

        debug::debug_dot(&*self.pipeline, "set_status_pipeline_main");
        for sink in &self.sinks {
            debug::debug_dot(
                &*sink.1.pipeline,
                format!("set_status_pipeline_{}", sink.0).as_str(),
            );
        }

        let current_stream = self
            .streams
            .get_mut(&descriptor)
            .context("failed to set state. Media stream with '{descriptors}' is missing")?;

        current_stream
            .audiomixer_sink()
            .context("unable to get sink for audiomixer")?
            .set_property("volume", if new_status.audio { 1.0 } else { 0.0 });
        current_stream.status = new_status;

        Ok(())
    }

    /// generate DOT file of the current pipeline
    ///
    /// # Arguments
    ///
    /// - `filename_without_extension`: Filename without extension.
    /// - `params`: Parameters of graph.
    ///
    pub fn dot(&self, filename_without_extension: &str, params: &debug::Params) {
        debug::dot_ext(&*self.pipeline, filename_without_extension, params);
    }

    /// Replace the current layout with the new one.
    ///
    /// # Errors
    ///
    /// This can fail if the `rerender_layout` function is failing.
    pub fn change_layout(&mut self, layout: impl Layout) -> Result<()> {
        self.layout = Box::new(layout);
        self.rerender_layout().context("unable to rerender layout")
    }

    /// Re-layout the current compositor scene.
    ///
    /// # Errors
    ///
    /// This can fail for the following reasons:
    /// - Pads cannot be retrieved.
    /// - Invalidate and validate for the bus monitor failed.
    pub fn rerender_layout(&mut self) -> Result<()> {
        if self.video_mixer.is_none() {
            // Doesn't need to rerender, if there is no compositor.
            return Ok(());
        };

        trace!(
            "layout({}): {}{}",
            self.output_resolution,
            if self.visibles.is_empty() {
                "(no visibles)"
            } else {
                ""
            },
            self.visibles
                .iter()
                .map(|v| format!("'{v}'"))
                .collect::<Vec<String>>()
                .join(",")
        );

        self.layout.set_resolution_changed(self.output_resolution);
        self.layout.set_amount_of_visibles(self.visibles.len());

        let mut streams = self.visibles.clone();

        let mut invisibles = self
            .streams
            .keys()
            .copied()
            .filter(|descriptor| !self.visibles.contains(descriptor))
            .collect();

        streams.append(&mut invisibles);

        // layout all video streams
        for (n, descriptor) in streams.iter().enumerate() {
            let stream = self.streams.get(descriptor).context("stream not found")?;
            if let Some(compositor_sink) = stream.compositor_sink() {
                if let Some(view) = self.layout.calculate_stream_view(n) {
                    compositor_sink.set_properties(&[
                        ("xpos", &(view.pos.x as i32).to_value()),
                        ("ypos", &(view.pos.y as i32).to_value()),
                        ("width", &(view.size.width as i32).to_value()),
                        ("height", &(view.size.height as i32).to_value()),
                        ("alpha", &(1.0).to_value()),
                    ]);
                    // Scale down the original video so the text overlay can be rendered properly
                    stream
                        .capsfilter()
                        .context("unable to get capsfilter for stream")?
                        .set_property(
                            "caps",
                            gst::Caps::builder("video/x-raw")
                                .field("width", view.size.width as i32)
                                .field("height", view.size.height as i32)
                                .field("pixel-aspect-ratio", Fraction::new(1, 1))
                                .build(),
                        );
                    // Reconfigure the videoconverscale after changing the size
                    stream
                        .videoconvertscale()
                        .context("unable to get videoconvertsccale for stream")?
                        .static_pad_with_context("src")?
                        .send_event(Reconfigure::new());
                } else {
                    compositor_sink.set_property("alpha", 0.0);
                }
            }
        }

        Ok(())
    }

    /// Add a callback function to the bus watch of the given sink.
    ///
    /// # Errors
    ///
    /// This can fail if the sink doesn't exists or if the callback cannot be
    /// added to the bus watch.
    pub fn add_watch_to_sink<F>(&self, name: &str, callback: F) -> Result<()>
    where
        F: FnMut(&Pipeline, MessageView) + Send + Sync + 'static,
    {
        let Some(active_sink) = self.sinks.get(name) else {
            bail!("there is no sink with the name {name}");
        };

        active_sink.pipeline.add_watch(callback);

        Ok(())
    }

    /// Get the current visible participants
    #[must_use]
    pub fn get_visibles(&self) -> HashSet<MediaDescriptor> {
        let mut visibles = self.visibles.clone();
        visibles.truncate(self.max_visibles);
        visibles.into_iter().collect::<HashSet<_>>()
    }
}

impl<SRC> Drop for Mixer<SRC>
where
    SRC: Source,
{
    fn drop(&mut self) {
        let streams = self
            .streams
            .values()
            .map(|stream| stream.bin.clone())
            .collect::<Vec<_>>();
        let mut mixer_pipeline = self.pipeline.clone_for_drop();
        let mut sink_pipelines = Vec::new();

        for active_sink in self.sinks.values_mut() {
            sink_pipelines.push(active_sink.pipeline.clone_for_drop());
        }

        tokio::task::block_in_place(move || {
            tokio::runtime::Handle::current().block_on(async move {
                for stream in streams {
                    if let Err(err) = stream.set_state(gst::State::Null) {
                        log::error!("unable to set stream to Null, reason: {err}");
                    }
                }

                for mut sink_pipeline in sink_pipelines {
                    sink_pipeline.drop().await;
                }

                mixer_pipeline.drop().await;
            });
        });
    }
}
