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
use gst::prelude::*;
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

/// Mixer managing the GStreamer pipeline using the given layout and source type
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
    compositor: gst::Element,
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
}

impl<SRC, STREAMID> Mixer<SRC, STREAMID>
where
    SRC: Source,
    STREAMID: Eq + Ord + Hash + Copy + Display + Debug + Sync + Send,
{
    /// Create a new mixer and setup the initial GStreamer pipeline with the given type of sink.
    ///
    /// # Arguments
    ///
    /// - `output_resolution`: Output video resolution.
    /// - `overlay`: List of overlays to attach behind the compositor
    /// - `sink_params`: Output sink parameters.
    ///
    pub fn new(output_resolution: Size, overlay: AnyOverlay, sink: impl Sink) -> Result<Self> {
        trace!("new( {output_resolution:?} )");

        // get width/height
        let width = output_resolution.width;
        let height = output_resolution.height;
        debug!(
            "New mixer output video ratio: {:.2} (= {width}/{height})",
            output_resolution.ratio()
        );

        // create new GStreamer pipeline
        let pipeline = gst::parse_launch(&format!(
            r#"
            
            videotestsrc
            name="Video Background Source"
            pattern=black
                is-live=true
            ! capssetter
                name="Video Background Capssetter"
                caps=video/x-raw,format=RGB,width={width},height={height}
            ! compositor
                name=video-compositor
                ignore-inactive-pads=true
                zero-size-is-unscaled=false
            ! queue
                name=video

            audiotestsrc
                name="Audio Background Source"
                is-live=true
                volume=0.0
            ! capssetter
                name="Audio Background Capssetter"
                caps=audio/x-raw,format=S16LE,channels=2,layout=interleaved,rate=48000
            ! audiomixer
                name=audio-mixer
                ignore-inactive-pads=true
            ! queue
                name=audio
            "#
        ))
        .expect("can not create pipeline");

        pipeline.set_property("name", "OpenTalk");

        let pipeline = pipeline
            .downcast::<gst::Pipeline>()
            .expect("not a pipeline");

        // get video elements from bin
        let compositor = pipeline
            .by_name("video-compositor")
            .expect("failed to get compositor from pipeline");
        let video_out = pipeline
            .by_name("video")
            .expect("failed to get video output from pipeline");
        let video_out_src = video_out
            .static_pad("src")
            .expect("failed to get source pad from video output");

        // get audio elements from bin
        let audio_mixer = pipeline
            .by_name("audio-mixer")
            .expect("failed to get audio mixer from pipeline");
        let audio_out = pipeline
            .by_name("audio")
            .expect("failed to ger audio output from pipeline");
        let audio_out_src = audio_out
            .static_pad("src")
            .expect("failed to get source pad from audio output");

        // create output sink to pipeline
        pipeline.add(&sink.bin())?;

        // add overlay to pipeline
        pipeline.add(overlay.element())?;

        // connect output pads to output sinks
        video_out_src
            .link(&overlay.sink())
            .expect("failed to link video output pad to overlay sink");
        overlay
            .src()
            .link(&sink.video())
            .expect("failed to link overlay src pad to video output sink");
        audio_out_src
            .link(&sink.audio())
            .expect("failed to link output pad to audio output sink");

        // start pipeline
        pipeline.set_state(gst::State::Playing)?;
        pipeline.sync_children_states()?;

        let (valid, valid_receiver) = std::sync::mpsc::channel::<Validation>();

        // pack all together
        let mut mixer = Mixer {
            compositor,
            audiomixer: audio_mixer,
            visibles: Vec::new(),
            pipeline,
            streams: HashMap::new(),
            overlay,
            output: Box::new(sink),
            output_resolution,
            valid,
        };

        // start reading the pipeline bus
        mixer.read_bus()?;
        mixer.monitor_layout(valid_receiver);

        // inform output sink that pipeline is are playing now
        mixer.output.on_play();

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
        let source = SRC::new(&id, params);

        let bin = gst::parse_bin_from_description(
            format!(
                r#"
            name="Overlay: {}"

            videoconvertscale
                name=videoconvertscale
            ! capsfilter
                name=capsfilter
            "#,
                id
            )
            .as_str(),
            false,
        )
        .expect("creation of bin failed");

        // add source to the bin
        bin.add(&source.bin()).expect("failed to add source to bin");

        // add overlay to the bin
        bin.add(overlay.element())?;

        bin.by_name("capsfilter")
            .expect("unable to get capfilter from bin")
            .static_pad("src")
            .expect("unable to get src of capsfilter")
            .link(&overlay.sink())
            .expect("unable to link the capsfilter to the overlay");

        // link source to overlay
        source
            .video()
            .link(
                &bin.by_name("videoconvertscale")
                    .expect("unable to get videoconvertscale from bin")
                    .static_pad("sink")
                    .expect("unable to get sink of videoconvertscale"),
            )
            .expect("could not link video source to overlay");

        let video = gst::GhostPad::with_target(Some("video"), &overlay.src())
            .expect("failed to create ghost pad for source video output");
        bin.add_pad(&video)
            .expect("failed to add video output ghost pad to source bin");

        // add the bin to the pipeline
        self.pipeline
            .add(&bin)
            .expect("failed to add source bin to pipeline");

        debug::debug_dot(&bin, "compositor_request_pad");

        let compositor_sink = self
            .compositor
            .request_pad_simple("sink_%u")
            .expect("could not get sink at compositor");
        compositor_sink.set_property_from_str("sizing-policy", "keep-aspect-ratio");
        video
            .link(&compositor_sink)
            .expect("could not connect video stream to compositor");

        // get audio source pad (no audio overlay yet)
        let audio_src = match source.bin().static_pad("audio") {
            Some(source_audio) => source_audio,
            _ => panic!("source's video pad is missing"),
        };

        let audio = gst::GhostPad::with_target(Some("audio"), &audio_src)
            .expect("failed to create ghost pad for source audio output");
        bin.add_pad(&audio)
            .expect("failed to add video output ghost pad to source bin");

        // link source's audio to audiomixer sink with the name of the stream ID
        let audiomixer_sink = self
            .audiomixer
            .request_pad_simple("sink_%u")
            .expect("could not get sink at audiomixer");
        audio
            .link(&audiomixer_sink)
            .expect("could not connect audio stream to audiomixer");

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
                        err.src().map(|s| s.path_string()),
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
                        warn.src().map(|s| s.path_string()),
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
                        info.src().map(|s| s.path_string()),
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
    ///
    pub fn state(&self) -> gst::State {
        self.pipeline.current_state()
    }

    /// remove an once added stream from the mixer.
    ///
    /// # Arguments
    ///
    /// - `id`: Unique identifier of the stream.
    ///
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
        self.compositor
            .release_request_pad(&stream.compositor_sink());
        self.audiomixer
            .release_request_pad(&stream.audiomixer_sink());

        // remove bin from pipeline
        stream.bin.set_state(gst::State::Null)?;

        self.pipeline
            .remove(&stream.bin)
            .expect("can not remove stream's bin from pipeline");

        // remove stream from visibles
        if let Some(index) = self.visibles.iter().position(|i| *i == id) {
            self.visibles.remove(index);
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
    pub fn show_stream(&mut self, id: &STREAMID, position_first: bool) {
        if self.is_visible(id) {
            return;
        }

        if position_first {
            self.visibles.insert(0, *id);
        } else {
            self.visibles.push(*id);
        }

        self.invalidate();
    }

    /// Set stream to the first position
    ///
    /// # Arguments
    ///
    /// `id`: ID of stream
    pub fn set_stream_to_first_position(&mut self, id: &STREAMID) {
        self.set_stream_to_position(id, 0);
    }

    /// Set stream to the first position
    ///
    /// # Arguments
    ///
    /// `id`: ID of stream
    pub fn set_stream_to_second_position(&mut self, id: &STREAMID) {
        self.set_stream_to_position(id, 1);
    }

    /// Set stream to the first position
    ///
    /// # Arguments
    ///
    /// `id`: ID of stream
    pub fn set_stream_to_position(&mut self, id: &STREAMID, position: usize) {
        if self.visibles.first() == Some(id) {
            return;
        }

        self.visibles.retain(|other_id| other_id != id);
        self.visibles.insert(position, *id);

        self.invalidate();
    }

    /// Hide stream.
    ///
    /// # Arguments
    ///
    /// `id`: ID of stream
    pub fn hide_stream(&mut self, id: &STREAMID) {
        if !self.is_visible(id) {
            return;
        }

        self.visibles.retain(|other_id| other_id != id);

        self.invalidate();
    }

    /// Return `true`, if stream is currently visible
    ///
    pub fn is_visible(&self, id: &STREAMID) -> bool {
        self.visibles.contains(id)
    }

    /// Return `true`, if stream currently provides video
    ///
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
    pub fn set_status(&mut self, id: &STREAMID, new_status: StreamStatus) -> Result<()> {
        info!("set_status( {id}, {new_status} )");

        debug::debug_dot(&self.pipeline, "set_status");

        let current_stream = self.get_stream_mut(id)?;
        current_stream
            .audiomixer_sink()
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
        debug::dot_ext(&self.pipeline, filename_without_extension, params)
    }

    fn invisibles(&self) -> Vec<STREAMID> {
        self.streams
            .keys()
            .cloned()
            .filter(|id| !self.visibles.contains(id))
            .collect()
    }

    /// Re-layout the current compositor scene.
    ///
    pub fn layout<L>(&mut self) -> Result<()>
    where
        L: Layout,
    {
        trace!(
            "layout<{}>({}): {}{}",
            L::NAME,
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

        // initialize the layout with current mixer setup
        let layout = L::new(self.visibles.len(), self.output_resolution);

        let mut streams = self.visibles.clone();
        streams.append(&mut self.invisibles());

        // layout all video streams
        for (n, id) in streams.iter().enumerate() {
            let stream = self.streams.get(id).expect("stream not found");
            let compositor_sink = stream.compositor_sink();
            if let Some(view) = layout.view(n) {
                compositor_sink.set_properties(&[
                    ("xpos", &(view.pos.x as i32).to_value()),
                    ("ypos", &(view.pos.y as i32).to_value()),
                    ("width", &(view.size.width as i32).to_value()),
                    ("height", &(view.size.height as i32).to_value()),
                    ("alpha", &(1.0).to_value()),
                ]);
                // Scale down the original video so the text overlay can be rendered properly
                stream.capsfilter().set_property(
                    "caps",
                    gst::Caps::builder("video/x-raw")
                        .field("width", view.size.width as i32)
                        .field("height", view.size.height as i32)
                        .build(),
                );
                // Reconfigure the videoconverscale after changing the size
                stream
                    .videoconvertscale()
                    .static_pad("src")
                    .expect("unable to get src from videoconvertscale")
                    .send_event(gst::event::Reconfigure::new());
            } else {
                compositor_sink.set_property("alpha", 0.0);
            }
        }

        self.validate();

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
    fn invalidate(&mut self) {
        trace!("invalidate()");

        self.valid
            .send(Validation::Invalid)
            .expect("cannot send layout invalidation");
    }

    fn validate(&self) {
        trace!("validate()");

        self.valid
            .send(Validation::Valid)
            .expect("cannot send layout validation");
    }

    fn monitor_layout(&self, receiver: std::sync::mpsc::Receiver<Validation>) {
        // monitor in a thread if `valid` will be set within latency timeout
        std::thread::spawn({
            move || {
                let mut valid = Validation::Valid;
                loop {
                    match valid {
                        Validation::Invalid => {
                            match receiver.recv_timeout(MAX_LAYOUT_UPDATE_LATENCY) {
                                Ok(v) => valid = v,
                                Err(_) => error!(
                                    "missing desired layout update since {duration}ms",
                                    duration = MAX_LAYOUT_UPDATE_LATENCY.as_millis()
                                ),
                            }
                        }
                        Validation::Valid => match receiver.recv() {
                            Ok(v) => valid = v,
                            Err(_) => todo!(),
                        },
                        Validation::Stop => break,
                    }
                }
            }
        });
    }
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
        assert_eq!(self.pipeline.current_state(), gst::State::Playing);

        debug!("Dropping mixer...");
        debug::debug_dot(&self.pipeline, "DROP");

        self.valid
            .send(Validation::Stop)
            .expect("could not stop validation monitor");

        // call sink to prepare for dropping pipeline
        debug!("Stop sink...");
        self.output.on_exit(&self.pipeline);

        // halt pipeline
        debug!("Nulling pipeline...");
        self.pipeline
            .set_state(gst::State::Null)
            .expect("Unable to set the pipeline to the `Null` state");

        debug!("Exited mixer.");
    }
}
