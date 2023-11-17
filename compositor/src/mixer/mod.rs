// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

//! Dynamic A/V mixer.

use anyhow::{Context, Result};

// sub-modules
pub mod debug;
mod overlay;
mod sink;
mod source;
mod stream;
mod talk;
mod text_style;

// forward useful sub-module stuff as public
pub use super::layout::*;
pub use overlay::*;
pub use sink::*;
pub use source::*;
pub use stream::*;
pub use talk::*;
pub use text_style::*;

// what we need from external libraries
use gst::{prelude::*, Caps, ElementFactory, Pipeline};
use std::{
    collections::HashMap,
    fmt::{Debug, Display},
    hash::Hash,
};

use anyhow::anyhow;

/// Maximum time a desired but missing re-layout is tolerated
const MAX_LAYOUT_UPDATE_LATENCY: std::time::Duration = std::time::Duration::from_millis(500);

enum Validation {
    Valid,
    Invalid,
    Stop,
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
pub struct Mixer<SRC, STREAMID>
where
    SRC: Source,
    STREAMID: Eq + Ord + Hash + Copy + Debug + Display,
{
    /// Current streams.
    streams: HashMap<STREAMID, Stream<SRC>>,
    /// Currently visible streams.
    visibles: Vec<STREAMID>,
    /// GStreamer element which composes the output video out of the source videos.
    compositor: Option<gst::Element>,
    /// GStreamer element which composes the output audio out of the source audios.
    audiomixer: gst::Element,
    /// The mixer GStreamer pipeline.
    pipeline: gst::Pipeline,
    /// Overlay behind compositor
    overlay: AnyOverlay,
    /// Holds the output sink.
    output: Box<dyn Sink>,
    /// over all generated output resolution
    output_resolution: Size,
    valid: std::sync::mpsc::Sender<Validation>,
    layout: Box<dyn Layout>,
}

impl<SRC, STREAMID> Mixer<SRC, STREAMID>
where
    SRC: Source,
    STREAMID: Eq + Ord + Hash + Copy + Display + Debug + Sync + Send,
{
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
    #[allow(clippy::too_many_lines)]
    pub fn create(
        pipeline: Pipeline,
        output_resolution: Size,
        layout: impl Layout,
        overlay: AnyOverlay,
        sink: impl Sink,
    ) -> Result<Self> {
        let audiotestsrc = ElementFactory::make("audiotestsrc")
            .name("Audio Background Source")
            .property("is-live", true)
            .property("volume", 0.0)
            .build()
            .context("unable to build audiotestsrc")?;
        let audio_capssetter = ElementFactory::make("capssetter")
            .name("Audio Background Capssetter")
            .property(
                "caps",
                Caps::builder("audio/x-raw")
                    .field("format", "S16LE")
                    .field("channels", 2)
                    .field("layout", "interleaved")
                    .field("rate", 48000)
                    .build(),
            )
            .build()
            .context("unable to build audio_capssetter")?;
        let audiomixer = ElementFactory::make("audiomixer")
            .name("audio-mixer")
            .property("ignore-inactive-pads", true)
            .build()
            .context("unable to build audiomixer")?;
        let audio_queue = ElementFactory::make("queue")
            .name("audio")
            .build()
            .context("unable to build audio_queue")?;

        pipeline
            .add_many(&[&audiotestsrc, &audio_capssetter, &audiomixer, &audio_queue])
            .context("unable to add audio elements to pipeline")?;

        let audio_requested_pad = audiomixer
            .request_pad_simple("sink_%u")
            .context("unable to request sink pad for audiomixer")?;

        audiotestsrc.link(&audio_capssetter)?;
        audio_capssetter
            .static_pad("src")
            .context("unable to get static pad src from audio_capssetter")?
            .link(&audio_requested_pad)
            .context("unable to link audio_requested_pad with audio_capssetter")?;
        audiomixer.link(&audio_queue)?;

        let compositor = if let Some(video_sink) = &sink.video() {
            let videotestsrc = ElementFactory::make("videotestsrc")
                .name("Video Background Source")
                .property_from_str("pattern", "black")
                .property("is-live", true)
                .build()
                .context("unable to build videotestsrc")?;
            let video_capssetter = ElementFactory::make("capssetter")
                .name("Video Background Capssetter")
                .property(
                    "caps",
                    Caps::builder("video/x-raw")
                        .field("format", "RGB")
                        .field("width", output_resolution.width as i32)
                        .field("height", output_resolution.height as i32)
                        .build(),
                )
                .build()
                .context("unable to build audio_capssetter")?;
            let compositor = ElementFactory::make("compositor")
                .name("video-compositor")
                .property("ignore-inactive-pads", true)
                .property("zero-size-is-unscaled", true)
                .build()
                .context("unable to build compositor")?;
            let video_queue = ElementFactory::make("queue")
                .name("video")
                .build()
                .context("unable to build video_queue")?;

            pipeline
                .add_many(&[&videotestsrc, &video_capssetter, &compositor, &video_queue])
                .context("unable to add video elements to pipeline")?;

            let video_requested_pad = compositor
                .request_pad_simple("sink_%u")
                .context("unable to request sink pad for compositor")?;

            videotestsrc.link(&video_capssetter)?;
            video_capssetter
                .static_pad("src")
                .context("unable to get static pad src from video_capssetter")?
                .link(&video_requested_pad)
                .context("unable to link video_requested_pad with video_capssetter")?;
            compositor.link(&video_queue)?;

            // get video elements from bin
            let compositor = pipeline
                .by_name("video-compositor")
                .context("failed to get compositor from pipeline")?;

            let video_out = pipeline
                .by_name("video")
                .context("failed to get video output from pipeline")?;
            let video_out_src = video_out
                .static_pad("src")
                .context("failed to get source pad from video output")?;

            // create output sink to pipeline
            pipeline
                .add(&sink.bin())
                .context("unable to add sink to pipeline")?;
            // add overlay to pipeline
            pipeline
                .add(overlay.element())
                .context("unable to add overlay to pipeline")?;

            // connect output pads to output sinks
            let overlay_sink = overlay.sink().context("unable to get sink for overlay")?;
            video_out_src
                .link(&overlay_sink)
                .context("failed to link video output pad to overlay sink")?;

            overlay
                .src()
                .context("unable to get src for overlay")?
                .link(video_sink)
                .context("failed to link overlay src pad to video output sink")?;

            Some(compositor)
        } else {
            // create output sink to pipeline
            pipeline
                .add(&sink.bin())
                .context("unable to add sink to pipeline")?;

            None
        };

        let audio_out_src = audio_queue
            .static_pad("src")
            .context("failed to get source pad from audio output")?;

        audio_out_src
            .link(&sink.audio())
            .context("failed to link output pad to audio output sink")?;

        pipeline.set_state(gst::State::Playing)?;
        pipeline.sync_children_states()?;

        let (valid, valid_receiver) = std::sync::mpsc::channel::<Validation>();

        let mut mixer = Mixer {
            compositor,
            audiomixer,
            visibles: Vec::new(),
            pipeline,
            streams: HashMap::new(),
            overlay,
            output: Box::new(sink),
            output_resolution,
            valid,
            layout: Box::new(layout),
        };

        // start reading the pipeline bus
        mixer.read_bus()?;
        monitor_layout(valid_receiver);

        // inform output sink that pipeline is are playing now
        mixer
            .output
            .on_play()
            .context("unable to call on_play output")?;

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
    #[allow(clippy::too_many_lines)]
    pub fn add_stream(
        &mut self,
        id: STREAMID,
        display_name: String,
        params: SRC::Parameters,
        overlay: AnyOverlay,
        status: StreamStatus,
    ) -> Result<()> {
        info!("add_stream( {id}, '{display_name}', {params:?} )");

        // check if stream ID is already known
        if self.streams.contains_key(&id) {
            warn!("Cannot add stream with ID {id} twice.");
            return Err(anyhow!("Cannot add stream with ID {id} twice."));
        }

        // create new source bin
        let source = SRC::create(&id, params).context("unable to create Source")?;

        let bin = if self.compositor.is_some() && source.video().is_some() {
            let description = format!(
                r#"
                    name="Overlay: {id}"

                    videoconvertscale
                        name=videoconvertscale
                    ! capsfilter
                        name=capsfilter
                "#
            );
            gst::parse_bin_from_description(&description, false)
                .context("creation of bin failed")?
        } else {
            gst::ElementFactory::make_with_name("bin", Some(&format!("Overlay: {id}")))
                .context(format!("unable to create bin with name 'Overlay: {id}'"))?
                .dynamic_cast::<gst::Bin>()
                .map_err(|element| {
                    anyhow!("unable to dynamic cast Elemenet '{element:?}' to gst::Bin")
                })
                .context("creation of bin failed")?
        };

        // add source to the bin
        bin.add(&source.bin())
            .context("failed to add source to bin")?;

        let video = if let Some(video) = source.video() {
            if self.compositor.is_some() {
                // add overlay to the bin
                bin.add(overlay.element())?;

                let overlay_sink = overlay.sink().context("unable to get sink for overlay")?;
                bin.by_name("capsfilter")
                    .context("unable to get capfilter from bin")?
                    .static_pad("src")
                    .context("unable to get src of capsfilter")?
                    .link(&overlay_sink)
                    .context("unable to link the capsfilter to the overlay")?;

                // link source to overlay
                video
                    .link(
                        &bin.by_name("videoconvertscale")
                            .context("unable to get videoconvertscale from bin")?
                            .static_pad("sink")
                            .context("unable to get sink of videoconvertscale")?,
                    )
                    .context("could not link video source to overlay")?;

                let overlay_src = overlay.src().context("unable to get src for overlay")?;
                let video = gst::GhostPad::with_target(Some("video"), &overlay_src)
                    .context("failed to create ghost pad for source video output")?;

                bin.add_pad(&video)
                    .context("failed to add video output ghost pad to source bin")?;

                Some(video)
            } else {
                let fakesink = gst::ElementFactory::make("fakesink").build()?;
                bin.add(&fakesink)
                    .context("unable to add `fakesink` to `bin`")?;
                let fakesink_sink_pad = fakesink
                    .static_pad("sink")
                    .context("unable to get static pad `sink` from `fakesink`")?;
                video
                    .link(&fakesink_sink_pad)
                    .context("failed to link video selector with target sink")?;
                fakesink
                    .sync_state_with_parent()
                    .context("unable to sync `fakesink` with parent")?;

                None
            }
        } else {
            None
        };

        // add the bin to the pipeline
        self.pipeline
            .add(&bin)
            .context("failed to add source bin to pipeline")?;

        debug::debug_dot(&bin, "compositor_request_pad");

        if let Some(compositor) = &self.compositor {
            let compositor_sink = compositor
                .request_pad_simple("sink_%u")
                .context("could not get sink at compositor")?;
            compositor_sink.set_property_from_str("sizing-policy", "keep-aspect-ratio");
            compositor_sink.set_property("alpha", 0.0);

            if let Some(video) = video.clone() {
                video
                    .link(&compositor_sink)
                    .context("could not connect video stream to compositor")?;
            }
        }

        // get audio source pad (no audio overlay yet)
        let audio_src = source
            .bin()
            .static_pad("audio")
            .context("source's video pad is missing")?;

        let audio = gst::GhostPad::with_target(Some("audio"), &audio_src)
            .context("failed to create ghost pad for source audio output")?;
        bin.add_pad(&audio)
            .context("failed to add video output ghost pad to source bin")?;

        // link source's audio to audiomixer sink with the name of the stream ID
        let audiomixer_sink = self
            .audiomixer
            .request_pad_simple("sink_%u")
            .context("could not get sink at audiomixer")?;
        audio
            .link(&audiomixer_sink)
            .context("could not connect audio stream to audiomixer")?;

        // sync state with rest of pipeline
        bin.sync_state_with_parent()?;

        trace!(
            "added stream {id}, {display_name:?}, {source},  {status:?}",
            source = debug::name(&source.bin()),
        );

        // remember the new A/V stream
        self.streams.insert(
            id,
            Stream {
                display_name,
                source,
                bin,
                video,
                audio,
                overlay,
                status,
            },
        );

        debug!("Added stream {id}");

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

    /// remove an once added stream from the mixer.
    ///
    /// # Arguments
    ///
    /// - `id`: Unique identifier of the stream.
    ///
    /// # Errors
    ///
    /// This can fail if the stream bin can't be set to NULL.
    pub fn remove_stream(&mut self, id: STREAMID) -> Result<()>
    where
        SRC: Source,
    {
        info!("remove_stream( {id} )");

        // remove stream from stored streams
        let stream = self
            .streams
            .remove(&id)
            .ok_or_else(|| anyhow!("given stream id ({id}) cannot be found"))?;

        trace!("releasing requested pads from mixers");
        // release video and audio sink pads
        if let Some(compositor) = &self.compositor {
            if let Some(compositor_sink) = &stream.compositor_sink() {
                compositor.release_request_pad(compositor_sink);
            }
        }
        let audiomixer_sink = stream
            .audiomixer_sink()
            .context("unable to get sink for audiomixer")?;
        self.audiomixer.release_request_pad(&audiomixer_sink);

        // remove bin from pipeline
        stream.bin.set_state(gst::State::Null)?;

        self.pipeline
            .remove(&stream.bin)
            .context("can not remove stream's bin from pipeline")?;

        // remove stream from visibles
        if let Some(index) = self.visibles.iter().position(|i| *i == id) {
            self.visibles.remove(index);
            self.rerender_layout()
                .context("unable to rerender layout")?;
        }

        debug!("Removed stream {id}");
        Ok(())
    }

    /// Show stream.
    ///
    /// # Arguments
    ///
    /// `id`: ID of stream
    /// `position_first`: Decides of the id should be pushed an the first or last position
    ///
    /// # Errors
    ///
    /// This can fail if the `rerender_layout` function is failing.
    pub fn show_stream(&mut self, id: &STREAMID, position_first: bool) -> Result<()> {
        if self.is_visible(id) {
            return Ok(());
        }

        if position_first {
            self.visibles.insert(0, *id);
        } else {
            self.visibles.push(*id);
        }
        self.rerender_layout().context("unable to rerender layout")
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
    pub fn set_stream_to_first_position(&mut self, id: &STREAMID) -> Result<()> {
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
    pub fn set_stream_to_second_position(&mut self, id: &STREAMID) -> Result<()> {
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
    pub fn set_stream_to_position(&mut self, id: &STREAMID, position: usize) -> Result<()> {
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
    pub fn hide_stream(&mut self, id: &STREAMID) -> Result<()> {
        if !self.is_visible(id) {
            return Ok(());
        }

        self.visibles.retain(|other_id| other_id != id);
        self.rerender_layout().context("unable to rerender layout")
    }

    /// Return `true`, if stream is currently visible
    ///
    pub fn is_visible(&self, id: &STREAMID) -> bool {
        self.visibles.contains(id)
    }

    /// Return `true`, if stream currently provides video
    ///
    /// # Errors
    ///
    /// This can fail if there is no stream with the given `id`.
    pub fn has_video(&self, id: &STREAMID) -> Result<bool> {
        Ok(self.get_stream(id)?.status.has_video)
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
    pub fn set_status(&mut self, id: &STREAMID, new_status: StreamStatus) -> Result<()> {
        info!("set_status( {id}, {new_status} )");

        debug::debug_dot(&self.pipeline, "set_status");

        let current_stream = self.get_stream_mut(id)?;
        current_stream
            .audiomixer_sink()
            .context("unable to get sink for audiomixer")?
            .set_property("volume", if new_status.has_audio { 1.0 } else { 0.0 });
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
    fn get_stream_mut(&mut self, id: &STREAMID) -> Result<&mut Stream<SRC>> {
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
    fn get_stream(&self, id: &STREAMID) -> Result<&Stream<SRC>> {
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

    fn invisibles(&self) -> Vec<STREAMID> {
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
        if self.compositor.is_none() {
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
                                .field("pixel-aspect-ratio", gst::Fraction::new(1, 1))
                                .build(),
                        );
                    // Reconfigure the videoconverscale after changing the size
                    stream
                        .videoconvertscale()
                        .context("unable to get videoconvertsccale for stream")?
                        .static_pad("src")
                        .context("unable to get src from videoconvertscale")?
                        .send_event(gst::event::Reconfigure::new());
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

impl<SRC, STREAMID> Drop for Mixer<SRC, STREAMID>
where
    SRC: Source,
    STREAMID: Eq + Ord + Hash + Copy + Debug + Display,
{
    /// halt pipeline (can not be played again)
    ///
    fn drop(&mut self) {
        // ensure playing

        debug!("Dropping mixer...");
        debug::debug_dot(&self.pipeline, "DROP");

        if let Err(error) = self.valid.send(Validation::Stop) {
            error!("could not stop validation monitor, error: {error}");
        }

        // call sink to prepare for dropping pipeline
        debug!("Stop sink...");
        if let Err(error) = self.output.on_exit(&self.pipeline) {
            error!("unable to call on_exit on output, error: {error}");
        }

        // halt pipeline
        debug!("Nulling pipeline...");
        if let Err(error) = self.pipeline.set_state(gst::State::Null) {
            error!("Unable to set the pipeline to the `Null` state, error: {error}");
        }

        debug!("Exited mixer.");
    }
}
