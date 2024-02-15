// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use anyhow::{anyhow, bail, Context, Result};
use gst::{
    event::Reconfigure, prelude::*, Bin, Clock, ClockTime, Element, ElementFactory, Fraction,
    GhostPad, Pipeline, SystemClock,
};
use gst_app::AppSrc;
use std::{
    collections::HashMap,
    fmt::{Debug, Display},
    hash::Hash,
};
use types::core::ParticipantId;
use types::signaling::media::{MediaSessionState, MediaSessionType};

mod audio_mixer;
pub mod debug;
mod overlay;
mod sink;
mod source;
mod stream;
mod text_style;
mod video_mixer;

use crate::{TalkOverlay, TextOverlay};

use self::{audio_mixer::AudioMixer, sink::ActiveSink, video_mixer::VideoMixer};

pub use super::layout::*;
pub use overlay::*;
pub use sink::*;
pub use source::*;
pub use stream::*;
pub use text_style::*;

/// Maximum time a desired but missing re-layout is tolerated
const MAX_LAYOUT_UPDATE_LATENCY: std::time::Duration = std::time::Duration::from_millis(500);

const AUDIO_SAMPLE_RATE: i32 = 48_000;
const AUDIO_CHANNELS: i32 = 2;

const VIDEO_WIDTH: i32 = 1920;
const VIDEO_HEIGHT: i32 = 1136;
const VIDEO_FRAMERATE: i32 = 30;

enum Validation {
    Valid,
    Invalid,
    Stop,
}

const NAME_FONT_SIZE: u32 = 16;

#[must_use]
pub fn media_types() -> impl DoubleEndedIterator<Item = MediaSessionType> {
    // order is priority for set speaker (first available will get focus)

    [MediaSessionType::Screen, MediaSessionType::Video].into_iter()
}

/// MediaDescriptor identifies a media stream by participant and media type.
#[derive(Debug, Hash, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MediaDescriptor {
    pub participant_id: ParticipantId,
    pub media_type: MediaSessionType,
}

impl Display for MediaDescriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "#{id:?}/{stream}",
            id = self.participant_id,
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
    visibles: Vec<MediaDescriptor>,
    audio_mixer: AudioMixer,
    video_mixer: Option<VideoMixer>,
    pipeline: Pipeline,
    overlay: AnyOverlay,
    sinks: HashMap<String, ActiveSink>,
    output_resolution: Size,
    valid: std::sync::mpsc::Sender<Validation>,
    layout: Box<dyn Layout>,
    system_clock: Clock,
    max_visibles: usize,
}

impl<SRC> Mixer<SRC>
where
    SRC: Source,
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

        Self::create(
            pipeline,
            resolution,
            layout,
            TalkOverlay::create()
                .context("unable to create TalkOverlay")?
                .into(),
            max_visibles,
            video_support,
        )
        .context("unable to create mixer")
    }

    /// Create a new mixer and setup the initial `GStreamer` pipeline with the given type of sink.
    ///
    /// # Arguments
    ///
    /// - `output_resolution`: Output video resolution.
    /// - `layout`: The layout which will be used.
    /// - `overlay`: List of overlays to attach behind the compositor
    /// - `sink_params`: Output sink parameters.
    ///
    /// # Errors
    ///
    /// This can fail if adding the pipeline and elements in `GStreamer` isn't working.
    pub fn create(
        pipeline: Pipeline,
        output_resolution: Size,
        layout: impl Layout,
        overlay: AnyOverlay,
        max_visibles: usize,
        video_support: bool,
    ) -> Result<Self> {
        let audio_mixer = AudioMixer::create().context("unable to create AudioMixer")?;
        pipeline
            .add(audio_mixer.bin())
            .context("unable to add 'audio_mixer' to 'pipeline'")?;

        let video_mixer = if video_support {
            let video_mixer = VideoMixer::create(output_resolution, &overlay)
                .context("unable to create VideoMixer")?;

            pipeline
                .add(video_mixer.bin())
                .context("unable to add 'video_mixer' to 'pipeline'")?;

            Some(video_mixer)
        } else {
            None
        };

        let system_clock = SystemClock::obtain();
        pipeline.use_clock(Some(&system_clock));
        pipeline.set_base_time(ClockTime::ZERO);
        pipeline.set_start_time(None);

        pipeline.set_state(gst::State::Playing)?;

        let sinks = HashMap::<String, ActiveSink>::new();

        let (valid, valid_receiver) = std::sync::mpsc::channel::<Validation>();

        let mut mixer = Mixer {
            audio_mixer,
            video_mixer,
            visibles: Vec::new(),
            pipeline,
            streams: HashMap::new(),
            overlay,
            sinks,
            output_resolution,
            valid,
            layout: Box::new(layout),
            system_clock,
            max_visibles,
        };

        // start reading the pipeline bus
        mixer.read_bus()?;
        monitor_layout(valid_receiver);

        Ok(mixer)
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
        id: MediaDescriptor,
        display_name: String,
        params: SRC::Parameters,
        initial: MediaSessionState,
    ) -> Result<()>
    where
        SRC: Source,
    {
        trace!("add_stream( {id}, '{display_name}', {params:?}, {initial} )");

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
        if self.streams.contains_key(&id) {
            warn!("Cannot add stream with ID {id} twice.");
            return Err(anyhow!("Cannot add stream with ID {id} twice."));
        }

        // create new source bin
        let source = SRC::create(&id, params).context("unable to create Source")?;
        let bin = Bin::new(Some(format!("Overlay: {id}").as_str()));

        // Add source bin to bin
        bin.add(&source.bin())
            .context("unable to add source bin to bin")?;

        // Setup video in pipeline
        if self.video_mixer.is_some() {
            if let Some(video) = source.video() {
                let videoconvertscale = ElementFactory::make("videoconvertscale")
                    .name("videoconvertscale")
                    .build()
                    .context("unable to build videoconvertscale")?;
                let capsfilter = ElementFactory::make("capsfilter")
                    .name("capsfilter")
                    .build()
                    .context("unable to build capsfilter")?;

                bin.add_many(&[&videoconvertscale, &capsfilter, overlay.element()])
                    .context(
                        "unable to add 'videoconvertscale', 'capsfilter' and 'overlay' to source bin",
                    )?;

                Element::link_many(&[&videoconvertscale, &capsfilter, &overlay.element()])
                    .context("unable to link 'videoconvertscale' and 'capsfilter")?;

                let videoconvertscale_sink_pad = videoconvertscale
                    .static_pad("sink")
                    .context("unable to get sink pad from videoconvertscale")?;
                video
                    .link(&videoconvertscale_sink_pad)
                    .context("unable to link video_src to videoconvertscale")?;
            }
        }

        // Add bin to pipeline
        self.pipeline
            .add(&bin)
            .context("failed to add source bin to pipeline")?;

        // Link audio in pipeline
        let audio_ghost_pad = GhostPad::with_target(None, &source.audio())
            .context("unable to create 'GhostPad' for 'audio'")?;
        bin.add_pad(&audio_ghost_pad)
            .context("unable to add audio_ghost_pad to bin")?;
        let audio = self
            .audio_mixer
            .link_src(&audio_ghost_pad)
            .context("unable to add 'audio' pad to 'audio_mixer'")?;

        // Link video in pipeline
        let video = if let (Some(video_mixer), Some(_)) = (&self.video_mixer, source.video()) {
            let overlay_src_pad = overlay
                .src()
                .context("unable to get src pad from overlay")?;
            let overlay_ghost_pad = GhostPad::with_target(None, &overlay_src_pad)
                .context("unable to create 'GhostPad' for 'videoconvertscale'")?;

            bin.add_pad(&overlay_ghost_pad)
                .context("unable to add overlay_ghost_pad to bin")?;

            let video = video_mixer
                .link_src(&overlay_ghost_pad)
                .context("unable to add 'video' pad to 'video_mixer'")?;

            Some(video)
        } else {
            None
        };

        debug::debug_dot(&self.pipeline, "stream_added");

        bin.sync_state_with_parent()
            .context("unable to sync state with parent for bin")?;

        self.streams.insert(
            id,
            Stream {
                display_name,
                source,
                bin,
                video,
                audio,
                overlay: AnyOverlay::Text(overlay),
                status: initial,
            },
        );

        debug!("Added stream {id}");

        // if available turn on audio but leave video off until `set_visibles()` is used
        self.set_stream_status(&id, initial)?;

        Ok(())
    }

    /// remove an once added stream from the mixer.
    ///
    /// # Arguments
    ///
    /// - `id`: Unique identifier of the stream.
    ///
    /// # Errors
    ///
    /// This can fail if the stream bin can't be set to NULL.
    pub fn remove_stream(&mut self, id: MediaDescriptor) -> Result<()> {
        trace!("remove_stream( {id} )");

        // remove stream from stored streams
        let stream = self
            .streams
            .remove(&id)
            .ok_or_else(|| anyhow!("given stream id ({id}) cannot be found"))?;

        // remove bin from pipeline
        stream.bin.set_state(gst::State::Null)?;

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
        if let Some(index) = self.visibles.iter().position(|i| *i == id) {
            self.visibles.remove(index);
            self.rerender_layout()
                .context("unable to rerender layout")?;
        }

        // After removing push the next screen share in the list to the first
        // position
        if let Some(stream_id) = self.get_first_screen_capture() {
            self.set_stream_to_first_position(&stream_id)
                .context("unable to set stream with id '{stream_id}' to first position")?;
        }

        Ok(())
    }

    /// Check if a given stream ID is known by the mixer.
    ///
    /// # Arguments
    ///
    /// - `id`: Describes which stream to search for.
    ///
    pub fn contains_stream(&self, id: &MediaDescriptor) -> bool {
        // forward to mixer
        self.streams.contains_key(id)
    }

    /// Get mutable access tp the internal stream with the given `id`.
    pub fn stream_mut(&mut self, id: &MediaDescriptor) -> Option<&mut Stream<SRC>> {
        // forward to mixer
        self.streams.get_mut(id)
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

        let stream_id = MediaDescriptor {
            participant_id: speaker,
            media_type: MediaSessionType::Screen,
        };
        if let Some(stream) = self.streams.get(&stream_id) {
            // The speaker has no screen, so it doesn't need to update the position
            if stream.status.video {
                self.set_stream_to_first_position(&stream_id)
                    .context("unable to set stream with id '{stream_id}' to first position")?;
            }
        }

        let stream_id = MediaDescriptor {
            participant_id: speaker,
            media_type: MediaSessionType::Video,
        };
        if let Some(stream) = self.streams.get(&stream_id) {
            // The speaker has no screen, so it doesn't need to update the position
            if stream.status.video {
                // check if noone is sharing their screen or the new speaker is also screen sharing
                if self.get_first_screen_capture().is_none() {
                    self.set_stream_to_first_position(&stream_id)
                        .context("unable to set stream with id '{stream_id}' to first position")?;
                } else {
                    self.set_stream_to_second_position(&stream_id)
                        .context("unable to set stream with id '{stream_id}' to second position")?;
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
        id: &MediaDescriptor,
        new_status: &MediaSessionState,
    ) -> Result<()> {
        info!("set_status({id}, {new_status:?}");
        let Some(current_stream) = self.streams.get(id) else {
            debug!("current_stream not found for id: {id:?}");
            return Ok(());
        };
        let old_status = current_stream.status;

        self.set_stream_status(id, *new_status)?;

        match (old_status.video, new_status.video) {
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
        if let AnyOverlay::Talk(overlay) = &self.overlay {
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
        if let AnyOverlay::Talk(overlay) = &self.overlay {
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
        if let AnyOverlay::Talk(overlay) = &self.overlay {
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
    pub fn set_stream_title(&self, id: &MediaDescriptor, title: &str) -> Result<()> {
        if let Some(stream) = self.streams.get(id) {
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
    pub fn show_stream(&mut self, stream_id: &MediaDescriptor) -> Result<()> {
        // Check if the maximum amount of streams is reached
        if self.visibles.len() >= self.max_visibles {
            // If the new stream is just a camera feed, then don't show them
            if stream_id.media_type == MediaSessionType::Video {
                return Ok(());
            }
            // The new camera feed is a screen share, which has a higher
            // priority, so the latest stream will be removed
            if let Some(id) = self.visibles.last().copied() {
                self.hide_stream(&id)
                    .context("unable hide stream for id '{id}'")?;
            }
        }
        // Check if the new stream is a screen capture
        // If it's a screen capture and noone else is streaming, push it to the first position
        // If someone is streaming, but the current speaker is the same user, push it to the first position
        let position_first = stream_id.media_type == MediaSessionType::Screen
            && self.get_first_screen_capture().is_none();

        if self.is_visible(stream_id) {
            return Ok(());
        }

        if position_first {
            self.visibles.insert(0, *stream_id);
        } else {
            self.visibles.push(*stream_id);
        }
        self.rerender_layout().context("unable to rerender layout")
    }

    /// Get mutable access to a source specified by stream ID.
    ///
    /// # Arguments
    ///
    /// - `id`: Describes which stream shall be returned.
    ///
    pub fn get_source(&mut self, id: &MediaDescriptor) -> Option<&mut SRC> {
        self.streams.get_mut(id).map(|stream| &mut stream.source)
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
    pub fn link_sink(&mut self, name: &str, mut sink: impl Sink) -> Result<()> {
        trace!("link sink, name: {name}, sinke: {sink:?}");
        if self.sinks.contains_key(name) {
            bail!("a stream with the name '{name}' already exists");
        }

        let pipeline = Pipeline::new(Some(name));

        pipeline.use_clock(Some(&self.system_clock));
        pipeline.set_base_time(ClockTime::ZERO);
        pipeline.set_start_time(None);

        let bin = sink.bin();
        pipeline
            .add(&bin)
            .context("unable to add sink to pipeline")?;

        self.link_audio_sink(&pipeline, &sink)
            .context("unable to link audio sink")?;
        self.link_video_sink(&pipeline, &sink)
            .context("unable to link video sink")?;

        pipeline
            .set_state(gst::State::Playing)
            .context("unable to start sink pipeline")?;
        pipeline
            .sync_children_states()
            .context("unable to sync children states for pipeline")?;

        sink.on_play().context("unable to set sink to playing")?;

        debug::dot(&self.pipeline, "link-sink-main-pipeline");
        debug::dot(
            &pipeline,
            format!("link-sink_sink-pipeline_{name}").as_str(),
        );

        let sink_state = ActiveSink {
            pipeline,
            sink: Box::new(sink),
        };

        self.sinks.insert(name.to_owned(), sink_state);

        Ok(())
    }

    /// Link the given sink to the `audio_mixer`.
    ///
    /// # Errors
    ///
    /// This can fail if the audio sink could not be linked to the `audio_mixer`.
    fn link_audio_sink(&self, pipeline: &Pipeline, sink: &impl Sink) -> Result<()> {
        let app_src = AppSrc::builder()
            .name("audiosrc")
            .caps(
                &gst::Caps::builder("audio/x-raw")
                    .field("format", "S16LE")
                    .field("layout", "interleaved")
                    .field("rate", AUDIO_SAMPLE_RATE)
                    .field("channels", AUDIO_CHANNELS)
                    .build(),
            )
            .min_latency(200_000_000i64)
            .format(gst::Format::Time)
            .max_bytes(1)
            .block(true)
            .is_live(true)
            .build();
        let queue = ElementFactory::make("queue")
            .property_from_str("leaky", "downstream")
            .build()
            .context("unable to create queue")?;
        let audioconvert = ElementFactory::make("audioconvert")
            .build()
            .context("unable to create audioconvert")?;

        pipeline
            .add_many(&[app_src.upcast_ref(), &queue, &audioconvert])
            .context("unable to add appsrc, queue and audioconvert to pipeline")?;

        Element::link_many(&[app_src.upcast_ref(), &queue, &audioconvert])
            .context("unable to link appsrc, queue and audioconvert ")?;

        audioconvert
            .static_pad("src")
            .context("unable to get static pad src from queue")?
            .link(&sink.audio())
            .context("unable to link queue with audio sink")?;

        self.audio_mixer.link_sink(&app_src);

        Ok(())
    }

    /// Link the given sink to the `video_mixer`.
    ///
    /// # Errors
    ///
    /// This can fail if the video sink could not be linked to the `video_mixer`.
    fn link_video_sink(&self, pipeline: &Pipeline, sink: &impl Sink) -> Result<()> {
        let Some(video_mixer) = &self.video_mixer else {
            return Ok(());
        };
        let Some(video_sink) = &sink.video() else {
            return Ok(());
        };

        let app_src = AppSrc::builder()
            .name("videosrc")
            .caps(
                &gst::Caps::builder("video/x-raw")
                    .field("format", "RGB")
                    .field("width", VIDEO_WIDTH)
                    .field("height", VIDEO_HEIGHT)
                    .field("framerate", Fraction::new(VIDEO_FRAMERATE, 1))
                    .build(),
            )
            .min_latency(200_000_000i64)
            .format(gst::Format::Time)
            .max_bytes(1)
            .block(true)
            .is_live(true)
            .build();
        let queue = ElementFactory::make("queue")
            .property_from_str("leaky", "downstream")
            .build()
            .context("unable to create queue")?;
        let videoconvert = ElementFactory::make("videoconvert")
            .build()
            .context("unable to create videoconvert")?;

        pipeline
            .add_many(&[app_src.upcast_ref(), &queue, &videoconvert])
            .context("unable to add appsrc, queue and videoconvert to pipeline")?;

        Element::link_many(&[app_src.upcast_ref(), &queue, &videoconvert])
            .context("unable to link appsrc, queue and videoconvert")?;

        videoconvert
            .static_pad("src")
            .context("unable to get static pad src from videoconvert")?
            .link(video_sink)
            .context("unable to link queue with video sink")?;

        video_mixer.link_sink(&app_src);

        Ok(())
    }

    /// Release the given sink from the mixer.
    ///
    /// # Errors
    ///
    /// This can fail if the sink could not be released from the mixer.
    pub fn release_sink(&mut self, name: &String) -> Result<()> {
        let Some(active_sink) = self.sinks.get_mut(name) else {
            bail!("there is no stream with the name '{name}'");
        };

        let audio_src = active_sink
            .pipeline
            .by_name("audiosrc")
            .context("unable to find audiosrc in sink pipeline")?;
        let audio_src: &AppSrc = audio_src
            .downcast_ref()
            .context("unable to downcast appsrc element to AppSrc")?;
        audio_src
            .end_of_stream()
            .context("unable to send EOS to audio_src")?;

        if self.video_mixer.is_some() {
            let video_src = active_sink
                .pipeline
                .by_name("videosrc")
                .context("unable to find audiosrc in sink pipeline")?;
            let video_src: &AppSrc = video_src
                .downcast_ref()
                .context("unable to downcast videosrc element to AppSrc")?;
            video_src
                .end_of_stream()
                .context("unable to send EOS to audio_src")?;
        }

        active_sink
            .sink
            .on_exit(&self.pipeline)
            .with_context(|| format!("unable to exit sink '{name}'"))?;

        self.sinks
            .remove(name)
            .with_context(|| format!("unable to remove sink '{name}' from sinks"))?;

        Ok(())
    }

    /// Continuously read the bus for errors and EOS.
    fn read_bus(&mut self) -> Result<()> {
        // get pipeline bus
        let bus = self
            .pipeline
            .bus()
            .context("failed to get bus of pipeline")?;

        // add watch which continuous recalculates latency
        let pipeline_weak = self.pipeline.downgrade();
        bus.add_watch(move |_, msg| {
            use gst::MessageView;
            // check several message types
            match (msg.view(), &pipeline_weak.upgrade()) {
                (MessageView::Error(err), Some(pipeline)) => {
                    error!(
                        "Error received from element {:?}: {}",
                        err.src().map(GstObjectExt::path_string),
                        err.error(),
                    );
                    debug::dot(pipeline, "BUS-ERROR");
                    if let Some(info) = err.debug() {
                        debug!("Debugging information: {}", info);
                    }
                }
                (MessageView::Warning(warn), Some(pipeline)) => {
                    warn!(
                        "Warning received from element {:?}: {}",
                        warn.src().map(GstObjectExt::path_string),
                        warn.error(),
                    );
                    debug::dot(pipeline, "BUS-WARNING");
                    if let Some(info) = warn.debug() {
                        debug!("Debugging information: {}", info);
                    }
                }
                (MessageView::Info(info), Some(pipeline)) => {
                    info!(
                        "Info received from element {:?}: {}",
                        info.src().map(GstObjectExt::path_string),
                        info.error(),
                    );
                    debug::dot(pipeline, "BUS-INFO");
                    if let Some(info) = info.debug() {
                        debug!("Debugging information: {}", info);
                    }
                }
                (MessageView::Latency(_), Some(pipeline)) => {
                    // Recalculate pipeline latency when requested
                    let _ = pipeline.recalculate_latency();
                }
                _ => (),
            }
            // stop reading if we are expecting EOS after the following scan
            Continue(true)
        })?;

        Ok(())
    }

    /// Return current pipeline state.
    #[must_use]
    pub fn state(&self) -> gst::State {
        self.pipeline.current_state()
    }

    /// Set stream to the first position
    ///
    /// # Arguments
    ///
    /// `id`: ID of stream
    ///
    /// # Errors
    ///
    /// This can fail if the `set_stream_to_position` function is failing.
    pub fn set_stream_to_first_position(&mut self, id: &MediaDescriptor) -> Result<()> {
        self.set_stream_to_position(id, 0)
    }

    /// Set stream to the first position
    ///
    /// # Arguments
    ///
    /// `id`: ID of stream
    ///
    /// # Errors
    ///
    /// This can fail if the `set_stream_to_position` function is failing.
    pub fn set_stream_to_second_position(&mut self, id: &MediaDescriptor) -> Result<()> {
        self.set_stream_to_position(id, 1)
    }

    /// Set stream to the first position
    ///
    /// # Arguments
    ///
    /// `id`: ID of stream
    ///
    /// # Errors
    ///
    /// This can fail if the `rerender_layout` function is failing.
    pub fn set_stream_to_position(&mut self, id: &MediaDescriptor, position: usize) -> Result<()> {
        if self.visibles.first() == Some(id) {
            return Ok(());
        }

        self.visibles.retain(|other_id| other_id != id);
        self.visibles.insert(position, *id);
        self.rerender_layout().context("unable to rerender layout")
    }

    /// Hide stream.
    ///
    /// # Arguments
    ///
    /// `id`: ID of stream
    ///
    /// # Errors
    ///
    /// This can fail if the `rerender_layout` function is failing.
    pub fn hide_stream(&mut self, id: &MediaDescriptor) -> Result<()> {
        if !self.is_visible(id) {
            return Ok(());
        }

        self.visibles.retain(|other_id| other_id != id);
        self.rerender_layout().context("unable to rerender layout")
    }

    /// Return `true`, if stream is currently visible
    ///
    pub fn is_visible(&self, id: &MediaDescriptor) -> bool {
        self.visibles.contains(id)
    }

    /// Return `true`, if stream currently provides video
    ///
    /// # Errors
    ///
    /// This can fail if there is no stream with the given `id`.
    pub fn has_video(&self, id: &MediaDescriptor) -> Result<bool> {
        self.get_stream(id).map(|stream| stream.status.video)
    }

    /// Set status of a stream.
    ///
    /// This function does not change visibility of a stream but audio presence.
    ///
    /// # Arguments
    ///
    /// - `id`: Describes which stream shall be updated.
    /// - `new_status`: New status to override.
    ///
    /// # Errors
    ///
    /// This can fail if the stream isn't in the `streams` list.
    pub fn set_stream_status(
        &mut self,
        id: &MediaDescriptor,
        new_status: MediaSessionState,
    ) -> Result<()> {
        info!("set_status( {id}, {new_status} )");

        debug::debug_dot(&self.pipeline, "set_status_pipeline_main");
        for sink in &self.sinks {
            debug::debug_dot(
                &sink.1.pipeline,
                format!("set_status_pipeline_{}", sink.0).as_str(),
            );
        }

        let current_stream = self.get_stream_mut(id)?;
        current_stream
            .audiomixer_sink()
            .context("unable to get sink for audiomixer")?
            .set_property("volume", if new_status.audio { 1.0 } else { 0.0 });
        current_stream.status = new_status;

        Ok(())
    }

    /// Access the mixer's mutable streams.
    ///
    /// # Arguments
    ///
    /// - `id`: ID of the stream.
    ///
    /// # Errors
    ///
    /// This can fail if the stream isn't in the `streams` list.
    fn get_stream_mut(&mut self, id: &MediaDescriptor) -> Result<&mut Stream<SRC>> {
        self.streams
            .get_mut(id)
            .ok_or_else(|| anyhow!("given stream id ({id}) cannot be found"))
    }

    /// Access the mixer's streams.
    ///
    /// # Arguments
    ///
    /// - `id`: ID of the stream.
    ///
    fn get_stream(&self, id: &MediaDescriptor) -> Result<&Stream<SRC>> {
        self.streams
            .get(id)
            .ok_or_else(|| anyhow!("given stream id ({id}) cannot be found"))
    }

    /// generate DOT file of the current pipeline
    ///
    /// # Arguments
    ///
    /// - `filename_without_extension`: Filename without extension.
    /// - `params`: Parameters of graph.
    ///
    pub fn dot(&self, filename_without_extension: &str, params: &debug::Params) {
        debug::dot_ext(&self.pipeline, filename_without_extension, params);
    }

    fn invisibles(&self) -> Vec<MediaDescriptor> {
        self.streams
            .keys()
            .copied()
            .filter(|id| !self.visibles.contains(id))
            .collect()
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
        self.invalidate().context("unable to invalidate layout")?;

        self.layout.set_resolution_changed(self.output_resolution);
        self.layout.set_amount_of_visibles(self.visibles.len());

        let mut streams = self.visibles.clone();
        streams.append(&mut self.invisibles());

        // layout all video streams
        for (n, id) in streams.iter().enumerate() {
            let stream = self.streams.get(id).context("stream not found")?;
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
                        .static_pad("src")
                        .context("unable to get src from videoconvertscale")?
                        .send_event(Reconfigure::new());
                } else {
                    compositor_sink.set_property("alpha", 0.0);
                }
            }
        }

        self.validate().context("unable to validate layout")?;

        Ok(())
    }

    /// Signal that layout has to be renewed from here
    ///
    /// Also checks if layout will be done within `MAX_LAYOUT_UPDATE_LATENCY`
    /// time and logs error if timeout was exceeded.
    /// This is to prevent any missed `layout()` after changing streams.
    /// Could be automatic but renewing the layout on every change leads to
    /// flickering in the output.
    ///
    fn invalidate(&mut self) -> Result<()> {
        trace!("invalidate()");

        self.valid
            .send(Validation::Invalid)
            .context("cannot send layout invalidation")
    }

    fn validate(&self) -> Result<()> {
        trace!("validate()");

        self.valid
            .send(Validation::Valid)
            .context("cannot send layout validation")
    }
}

fn monitor_layout(receiver: std::sync::mpsc::Receiver<Validation>) {
    // monitor in a thread if `valid` will be set within latency timeout
    std::thread::spawn({
        move || {
            let mut valid = Validation::Valid;
            loop {
                match valid {
                    Validation::Invalid => {
                        if let Ok(v) = receiver.recv_timeout(MAX_LAYOUT_UPDATE_LATENCY) {
                            valid = v;
                        } else {
                            error!(
                                "missing desired layout update since {duration}ms",
                                duration = MAX_LAYOUT_UPDATE_LATENCY.as_millis()
                            );
                        }
                    }
                    Validation::Valid => match receiver.recv() {
                        Ok(v) => valid = v,
                        Err(error) => {
                            error!("unable to receive valid Validation in monitor_layout, error: {error}");
                        }
                    },
                    Validation::Stop => break,
                }
            }
        }
    });
}

impl<SRC> Drop for Mixer<SRC>
where
    SRC: Source,
{
    /// halt pipeline (can not be played again)
    ///
    fn drop(&mut self) {
        debug!("Dropping Mixer...");
        debug::debug_dot(&self.pipeline, "MIXER-DROP");

        trace!("remove_all_stream()");
        let ids: Vec<MediaDescriptor> = self.streams.keys().copied().collect();
        for id in ids {
            if let Err(error) = self.remove_stream(id) {
                error!("could not remove stream, error: {error}");
            }
        }

        if let Err(error) = self.valid.send(Validation::Stop) {
            error!("could not stop validation monitor, error: {error}");
        }

        debug!("Nulling pipeline...");
        if let Err(error) = self.pipeline.set_state(gst::State::Null) {
            error!("Unable to set the pipeline to the `Null` state, error: {error}");
        }

        debug!("Exited mixer.");
    }
}
