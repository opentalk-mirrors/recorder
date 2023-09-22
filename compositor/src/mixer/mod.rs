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
use std::sync::atomic::{AtomicBool, Ordering};
pub use stream::*;
pub use talk::*;
pub use text_style::*;

// what we need from external libraries
use gst::prelude::*;
use std::{
    collections::HashMap,
    fmt::{Debug, Display},
    hash::Hash,
    sync::{Arc, Condvar, Mutex},
};

use anyhow::anyhow;

macro_rules! log_err_like {
    ($name:expr,$error:expr,$pipeline:expr) => {{
        error!(
            "{name} received from element {:?}: {}",
            $error.src().map(|s| s.path_string()),
            $error.error(),
            name = $name,
        );
        debug::dot($pipeline, "BUS-ERROR");
        if let Some(info) = $error.debug() {
            debug!("Debugging information: {}", info);
        }
    }};
}

/// Maximum time a desired but missing re-layout is tolerated
const MAX_LAYOUT_UPDATE_LATENCY: std::time::Duration = std::time::Duration::from_millis(100);
/// Time to wait for EOS when dropping the mixer
const BUS_EOS_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(2000);

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
pub struct Mixer<SRC, ID>
where
    SRC: Source + Debug,
    SRC::Parameters: Debug,
    ID: Eq + Ord + Hash + Copy + Debug + Display,
{
    /// Current streams.
    streams: HashMap<ID, Stream<SRC>>,
    /// Currently visible streams.
    visibles: Vec<ID>,
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
    /// signals when bus reading stops
    is_reading_bus: Option<std::sync::mpsc::Receiver<bool>>,
    /// needs layout if false
    valid: Arc<(Mutex<bool>, Condvar)>,
    /// if true we expect to get an EOS on bus
    expect_eos: Arc<AtomicBool>,
}

impl<SRC, ID> Mixer<SRC, ID>
where
    SRC: Source + Debug,
    SRC::Parameters: Debug,
    ID: Eq + Ord + Hash + Copy + Display + Debug + Sync + Send,
{
    /// Create a new mixer and setup the initial GStreamer pipeline with the given type of sink.
    ///
    /// # Arguments
    ///
    /// - `output_resolution`: Output video resolution.
    /// - `overlay`: List of overlays to attach behind the compositor
    /// - `sink_params`: Output sink apramaters.
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

        // create output sink
        pipeline.add(&sink.bin())?;

        pipeline.add(overlay.element())?;

        debug::dot(&pipeline, "new");
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
            is_reading_bus: None,
            valid: Arc::new((Mutex::new(false), Condvar::new())),
            expect_eos: Arc::new(AtomicBool::new(false)),
        };

        // start reading the pipeline bus
        mixer.read_bus()?;

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
        id: ID,
        display_name: String,
        params: SRC::Parameters,
        overlay: AnyOverlay,
        status: StreamStatus,
    ) -> Result<()> {
        trace!("add_stream( {id}, '{display_name}', {params:?} )");

        // check if stream ID is already known
        if self.streams.contains_key(&id) {
            warn!("Cannot add stream with ID {id} twice.");
            return Err(anyhow!("Cannot add stream with ID {id} twice."));
        }

        // create new source bin
        let source = SRC::new(&id, params);

        // create a bin which will include the source and the overlay
        let overlay_bin =
            gst::ElementFactory::make_with_name("bin", Some(&format!("Overlay: {id}")))?
                .dynamic_cast::<gst::Bin>()
                .expect("creation of bin failed");

        // add source to the bin
        overlay_bin
            .add(&source.bin())
            .expect("failed to add source to bin");

        // add overlay to the bin
        overlay_bin.add(overlay.element())?;

        // link source to overlay
        source
            .video()
            .link(&overlay.sink())
            .expect("could not link video source to overlay");

        let video_src = gst::GhostPad::with_target(Some("video"), &overlay.src())
            .expect("failed to create ghost pad for source video output");
        overlay_bin
            .add_pad(&video_src)
            .expect("failed to add video output ghost pad to source bin");

        // add the bin to the pipeline
        self.pipeline
            .add(&overlay_bin)
            .expect("failed to add source bin to pipeline");

        debug::dot(&overlay_bin, "compositor_request_pad");

        let compositor_sink = self
            .compositor
            .request_pad_simple("sink_%u")
            .expect("could not get sink at compositor");
        compositor_sink.set_property_from_str("sizing-policy", "keep-aspect-ratio");
        video_src
            .link(&compositor_sink)
            .expect("could not connect video stream to compositor");

        // get audio source pad (no audio overlay yet)
        let audio_src = match source.bin().static_pad("audio") {
            Some(source_audio) => source_audio,
            _ => panic!("source's video pad is missing"),
        };

        let audio_src = gst::GhostPad::with_target(Some("audio"), &audio_src)
            .expect("failed to create ghost pad for source audio output");
        overlay_bin
            .add_pad(&audio_src)
            .expect("failed to add video output ghost pad to source bin");

        // link source's audio to audiomixer sink with the name of the stream ID
        let audiomixer_sink = self
            .audiomixer
            .request_pad_simple("sink_%u")
            .expect("could not get sink at audiomixer");
        audio_src
            .link(&audiomixer_sink)
            .expect("could not connect audio stream to audiomixer");

        // sync state with rest of pipeline
        overlay_bin.sync_state_with_parent()?;

        // remember the new A/V stream
        self.streams.insert(
            id,
            Stream::new(
                &id,
                display_name,
                source,
                overlay_bin,
                video_src,
                audio_src,
                overlay,
                status,
            ),
        );

        debug!("Added stream {id}");

        Ok(())
    }

    /// Continuously read the bus for errors and EOS.
    fn read_bus(&mut self) -> Result<()> {
        // abort if we already are reading the bus
        if self.is_reading_bus.is_some() {
            return Ok(());
        }
        // signal that we start reading the bus
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        self.is_reading_bus = Some(rx);

        // get pipeline bus
        let bus = self
            .pipeline
            .bus()
            .context("failed to get bus of pipeline")?;

        // add watch which continuous recalculates latency
        let pipeline_weak = self.pipeline.downgrade();
        let expect_eos = self.expect_eos.clone();
        bus.add_watch(move |_, msg| {
            use gst::MessageView;
            // check several message types
            match (msg.view(), &pipeline_weak.upgrade()) {
                (MessageView::Error(err), Some(pipeline)) => {
                    log_err_like!("Error", err, pipeline)
                }
                (MessageView::Warning(warn), Some(pipeline)) => {
                    log_err_like!("Warning", warn, pipeline)
                }
                (MessageView::Info(info), Some(pipeline)) => {
                    log_err_like!("Info", info, pipeline)
                }
                // check if EOS is one we send ourself and so is expected
                (MessageView::Eos(..), _) => {
                    if expect_eos.load(Ordering::SeqCst) {
                        debug!("got expected EOS");
                        tx.send(false).expect("could not send on sync channel");
                    } else {
                        error!("got unexpected EOS");
                        tx.send(true).expect("could not send on sync channel");
                    }
                }
                (MessageView::Latency(_), Some(pipeline)) => {
                    // Recalculate pipeline latency when requested
                    let _ = pipeline.recalculate_latency();
                }
                _ => (),
            }
            // stop reading if we are expecting EOS after the following scan
            Continue(!expect_eos.load(Ordering::SeqCst))
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
    pub fn remove_stream(&mut self, id: ID) -> Result<()> {
        trace!("remove_stream( {id} )");
        debug::debug_dot(&self.pipeline, "remove_stream");

        // remove stream from stored streams
        let stream = self
            .streams
            .remove(&id)
            .ok_or_else(|| anyhow!("given stream id ({id}) cannot be found"))?;

        // remove requested pads from mixers
        self.audiomixer
            .release_request_pad(&stream.audiomixer_sink());
        self.compositor
            .release_request_pad(&stream.compositor_sink());

        // remove bin from pipeline
        stream.bin.set_state(gst::State::Null)?;
        stream.bin.sync_children_states()?;

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

    /// Select the streams which are visible.
    ///
    /// All previously visible streams get invisible if they are not in the list.
    /// See set_speaker() for further info about how the order will be interpreted.
    ///
    /// # Arguments
    ///
    /// - `ids`: List of identifiers of streams which shall get visible
    ///
    pub fn set_visibles(&mut self, ids: &[ID]) {
        trace!("set_visibles( {ids:?} )");
        trace!("currently visible: {:?} ", self.visibles);

        // copy ID list of visibles
        self.visibles = ids.into();
        self.invalidate();

        debug!("set visibles to {:?}", self.visibles);
    }

    /// Set visibility of a participant.
    ///
    /// # Arguments
    ///
    /// `id`: ID of participant
    /// `visible`: Show if `true` otherwise hide.
    ///
    /// # Return
    ///
    /// - `false` if stream has been made visible.
    /// - `true` if max visibles was exceeded and stream could not be shown.
    ///
    pub fn set_visible(&mut self, id: &ID, visible: bool) -> bool {
        // only show if not already visible or vice versa
        match (visible, self.is_visible(id)) {
            (true, false) => {
                // Clone current visibles
                let mut ids = self.visibles.clone();
                // add stream to visibles
                debug!("show {id}");
                // add the new one
                ids.push(*id);
                // set new visibles
                self.set_visibles(&ids);
                // recalculate layout
                self.invalidate();
                false
            }
            (false, true) => {
                // Clone current visibles
                let mut ids = self.visibles.clone();
                // add stream to visibles
                debug!("hide {id}");
                // add the new one (self.is_visible(id)==true ensures success)
                ids.remove(ids.iter().position(|i| i == id).unwrap());
                // set new visibles
                self.set_visibles(&ids);
                // recalculate layout
                self.invalidate();
                false
            }
            (true, true) => {
                warn!("try to show already visible {id}");
                true
            }
            (false, false) => {
                warn!("try to hide already invisible {id}");
                true
            }
        }
    }

    /// Return `true`, if stream is currently visible
    ///
    pub fn is_visible(&self, id: &ID) -> bool {
        self.visibles.contains(id)
    }

    /// Return `true`, if stream currently provides video
    ///
    pub fn has_video(&self, id: &ID) -> Result<bool> {
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
    pub fn set_status(&mut self, id: &ID, new_status: StreamStatus) -> Result<()> {
        trace!("set_status( {id}, {new_status} )");

        // get old stream's status
        let stream = self
            .streams
            .get(id)
            .ok_or_else(|| anyhow!("given stream id ({id}) cannot be found"))?;

        debug::dot(&self.pipeline, "set_status");
        stream
            .audiomixer_sink()
            .set_property("volume", if new_status.has_audio { 1.0 } else { 0.0 });

        self.invalidate();

        // set stream's new status
        self.get_stream_mut(id)?.status = new_status;

        Ok(())
    }

    /// Access the mixer's mutable streams.
    ///
    /// # Arguments
    ///
    /// - `id`: ID of the stream.
    ///
    fn get_stream_mut(&mut self, id: &ID) -> Result<&mut Stream<SRC>> {
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
    fn get_stream(&self, id: &ID) -> Result<&Stream<SRC>> {
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

    fn invisibles(&self) -> Vec<ID> {
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
            let view = layout.view(n);
            stream.compositor_sink().set_properties(&[
                ("xpos", &(view.pos.x as i32).to_value()),
                ("ypos", &(view.pos.y as i32).to_value()),
                ("width", &(view.size.width as i32).to_value()),
                ("height", &(view.size.height as i32).to_value()),
                ("alpha", &(view.alpha).to_value()),
            ]);
        }

        // Signal that layout has been updated
        let (valid, condvar) = &*self.valid;
        if let Ok(mut valid) = valid.lock() {
            *valid = true;
        }
        condvar.notify_one();

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

        // warn if no visibles were set
        if self.visibles.is_empty() {
            warn!("No visibles in layout! Talk closed?");
        }

        // set valid to false
        if let Ok(mut valid) = self.valid.0.lock() {
            *valid = false;
        }

        // monitor in a thread if `valid` will be set within latency timeout
        std::thread::spawn({
            let valid = self.valid.clone();
            move || {
                let (valid, condvar) = &*valid;
                if let Ok(valid) = valid.lock() {
                    let result = condvar
                        .wait_timeout_while(valid, MAX_LAYOUT_UPDATE_LATENCY, |valid| !*valid)
                        .expect("invalid layout update latency timeout");
                    if result.1.timed_out() {
                        error!(
                            "missing desired layout update since {duration}ms",
                            duration = MAX_LAYOUT_UPDATE_LATENCY.as_millis()
                        )
                    }
                }
            }
        });
    }
}

impl<SRC, ID> Drop for Mixer<SRC, ID>
where
    SRC: Source + Debug,
    SRC::Parameters: Debug,
    ID: Eq + Ord + Hash + Copy + Debug + Display,
{
    /// halt pipeline (can not be played again)
    ///
    fn drop(&mut self) {
        // ensure playing
        assert_eq!(self.pipeline.current_state(), gst::State::Playing);

        debug!("Dropping mixer...");

        // send expected EOS
        debug!("Sending EOS...");
        self.expect_eos.store(true, Ordering::SeqCst);
        self.pipeline.send_event(gst::event::Eos::new());
        debug::debug_dot(&self.pipeline, "EOS");

        // wait until the bus reader thread got the EOS and finishes
        if let Some(is_reading_bus) = &self.is_reading_bus {
            // Print an error message if the bus could not handle the timeout in the given time.
            // If the pipeline is handling the EOS fast enough, the `is_reading_bus` queue will hung up early, which is a normal behaviour and not an error.
            if is_reading_bus.recv_timeout(BUS_EOS_TIMEOUT)
                == Err(std::sync::mpsc::RecvTimeoutError::Timeout)
            {
                error!("Could not stop reading pipeline bus");
            }
        }

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
