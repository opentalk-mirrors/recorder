use anyhow::{Context, Result};

// sub-modules
pub mod debug;
pub mod dynamic;
mod overlay;
mod sink;
mod source;
mod stream;
mod talk;
mod text_format;

// forward useful sub-module stuff as public
pub use super::layout::*;
pub use overlay::*;
pub use sink::*;
pub use source::*;
use std::sync::atomic::{AtomicBool, Ordering};
pub use stream::*;
pub use talk::*;
pub use text_format::*;

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
        debug::dot(&$pipeline, "BUS-ERROR");
        if let Some(info) = $error.debug() {
            debug!("Debugging information: {}", info);
        }
    }};
}

/// Maximum time a desired but missing re-layout is tolerated
const MAX_LAYOUT_UPDATE_LATENCY: std::time::Duration = std::time::Duration::from_millis(100);
/// Time to wait for EOS when dropping the mixer
const BUS_EOS_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(2000);
/// Period in which the bus is scanned (must be smaller than [BUS_EOS_TIMEOUT] )
const BUS_READ_PERIOD: gst::ClockTime = gst::ClockTime::from_mseconds(1000);

/// Mixer managing the GStreamer pipeline using the given layout and source type
/// # Types
/// - `SRC`: Source type to use when adding streams.
/// - `SINK`: Sink type to use for output.D
/// - `ID`: stream identifier type
#[derive(Debug)]
pub struct Mixer<SRC, ID>
where
    SRC: Source,
    SRC::Parameters: Debug,
    ID: Eq + Ord + Hash + Copy + Debug + Display,
{
    /// Current streams.
    pub streams: HashMap<ID, Stream<SRC>>,
    /// Number of currently visible streams.
    pub visibles: Vec<ID>,
    /// GStreamer element which composes the output video out of the source videos.
    compositor: gst::Element,
    /// GStreamer element which composes the output audio out of the source audios.
    audio_mixer: gst::Element,
    /// The mixer GStreamer pipeline.
    pub pipeline: gst::Pipeline,
    /// on top overlays
    overlays: Overlays,
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
    SRC: Source,
    SRC::Parameters: Debug,
    ID: Eq + Ord + Hash + Copy + Display + Debug + Sync + Send,
{
    /// Create a new mixer and setup the initial GStreamer pipeline with the given type of sink.
    ///
    /// # Arguments
    ///
    /// - `resolution`: Output video resolution.
    /// - `sink_params`: Parameters to create the output sink.
    ///
    pub fn new(resolution: Size, sink_builder: Box<dyn SinkBuilder>) -> Result<Self> {
        trace!("new( {resolution:?} )");

        // get width/height
        let width = resolution.width;
        let height = resolution.height;
        debug!(
            "New mixer output video ratio: {:.2} (= {width}/{height})",
            resolution.ratio()
        );

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
                    ! valve
                        name=valve-overlay
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
        let compositor = pipeline
            .by_name("video-compositor")
            .expect("failed to get compositor from pipeline");
        let video_out = pipeline
            .by_name("video-out")
            .expect("failed to get video output from pipeline");
        let video_output_pad = video_out
            .static_pad("src")
            .expect("failed to get source pad from video output");

        // get audio elements from bin
        let audio_mixer = pipeline
            .by_name("audio-mixer")
            .expect("failed to get audio mixer from pipeline");
        let audio_out = pipeline
            .by_name("audio-out")
            .expect("failed to ger audio output from pipeline");
        let audio_output_pad = audio_out
            .static_pad("src")
            .expect("failed to get source pad from audio output");

        // create output sink
        let output = sink_builder.as_ref().build(&pipeline);

        // connect output pads to output sinks
        video_output_pad
            .link(&output.video_sink_pad())
            .expect("failed to link output pad to video output sink");
        audio_output_pad
            .link(&output.audio_sink_pad())
            .expect("failed to link output pad to audio output sink");

        // create new overlays container
        let valve_overlay = pipeline
            .by_name("valve-overlay")
            .expect("failed to get video output valve from pipeline");

        let overlays = Overlays::new(valve_overlay);

        // start pipeline
        pipeline.set_state(gst::State::Playing)?;

        // pack all together
        let mut mixer = Mixer {
            compositor,
            audio_mixer,
            visibles: Vec::new(),
            overlays,
            pipeline,
            streams: HashMap::new(),
            output,
            output_resolution: resolution,
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
    /// # Arguments
    ///
    /// - `id`: Unique identifier of the stream.
    /// - `display_name`: Name to display to user as identifier.
    /// - `params`: Source specific parameters.
    ///
    pub fn add_stream(
        &mut self,
        id: ID,
        display_name: String,
        params: SRC::Parameters,
    ) -> Result<()> {
        trace!("add_stream( {id}, '{display_name}', {params:?} )");

        // check if stream ID is already known
        if self.streams.contains_key(&id) {
            return Err(anyhow!("tried to insert already existing ID ({id})"));
        }

        // add new stream
        let mut stream: Stream<SRC> = Stream::new(
            &id,
            &self.pipeline,
            &self.output_resolution,
            display_name,
            params,
        );

        // attach video source to a valve
        let valve =
            dynamic::add_source(&stream.source.bin(), &stream.source.video_out_pad(), None)?;
        stream.video_link_status = LinkStatus::Unlinked(valve);

        // attach audio source to a valve
        let valve =
            dynamic::add_source(&stream.source.bin(), &stream.source.audio_out_pad(), None)?;
        stream.audio_link_status = LinkStatus::Unlinked(valve);

        // start any not playing pipeline elements
        self.pipeline.set_state(gst::State::Playing)?;

        // remember the new A/V stream
        self.streams.insert(id, stream);

        debug!("Added stream {id}");

        Ok(())
    }

    /// Continuously read the bus for errors and EOS.
    fn read_bus(&mut self) -> Result<()> {
        // signal that we start reading the bus
        if self.is_reading_bus.is_some() {
            return Ok(());
        }
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        self.is_reading_bus = Some(rx);

        let bus = self
            .pipeline
            .bus()
            .context("failed to get bus of pipeline")?;

        std::thread::spawn({
            let pipeline = self.pipeline.clone();
            let expect_eos = self.expect_eos.clone();
            move || {
                debug!("Started to read the pipeline bus.");
                loop {
                    for msg in bus.iter_timed(BUS_READ_PERIOD) {
                        use gst::MessageView;
                        match msg.view() {
                            MessageView::Error(err) => log_err_like!("Error", err, pipeline),
                            MessageView::Warning(warn) => log_err_like!("Warning", warn, pipeline),
                            MessageView::Info(info) => log_err_like!("Info", info, pipeline),
                            MessageView::Eos(..) => {
                                if expect_eos.load(Ordering::SeqCst) {
                                    debug!("got expected EOS");
                                    tx.send(false).expect("could not send on sync channel");
                                    break;
                                } else {
                                    error!("got unexpected EOS");
                                    tx.send(true).expect("could not send on sync channel");
                                }
                                break;
                            }
                            _ => (),
                        }
                    }
                    // stop reading if we are expecting EOS after the following scan
                    if expect_eos.load(Ordering::SeqCst) {
                        break;
                    }
                }
            }
        });
        Ok(())
    }

    /// Return current pipeline state.
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

        // remove stream from stored streams
        let stream = self
            .streams
            .remove(&id)
            .ok_or_else(|| anyhow!("given stream id ({id}) cannot be found"))?;

        debug::debug_dot(&self.pipeline, "remove_stream");

        if let Some(valve) = stream.audio_link_status.valve() {
            if let Some(inp_pad) = stream.source.audio_inp_pad() {
                // unlink and remove audio source from pipeline
                dynamic::remove_source(inp_pad, &valve, &self.audio_mixer)?;
            }
            dynamic::remove_valve(valve)?;
        } else {
            error!("could not find valve of audio source of {id}")
        }

        // unlink and remove video source from pipeline
        if let Some(valve) = stream.video_link_status.valve() {
            if let Some(inp_pad) = stream.source.video_inp_pad() {
                // unlink and remove audio source from pipeline
                dynamic::remove_source(inp_pad, &valve, &self.compositor)?;
            }
            dynamic::remove_valve(valve)?;
        } else {
            error!("could not find valve of video source of {id}")
        }

        dynamic::remove_bin(stream.source.bin())?;

        // remove stream from visibles
        if let Some(index) = self.visibles.iter().position(|i| *i == id) {
            self.visibles.remove(index);
        }

        debug!("Removed stream {id}");
        Ok(())
    }

    /// push new overlay on top of output video within the pipeline
    ///
    /// # Arguments
    ///
    /// - `overlay`: new overlay to push
    ///
    pub fn insert_overlay(&mut self, overlay: Overlay) -> Result<()> {
        trace!("push_overlay( {overlay:?} )");

        // forward to overlays
        self.overlays.push(overlay)?;

        debug!("Pushed overlay");
        Ok(())
    }

    /// push new overlay on top of source video within the pipeline
    ///
    /// # Arguments
    ///
    /// - `overlay`: new overlay to push
    ///
    pub fn insert_source_overlay(&mut self, id: &ID, overlay: Overlay) -> Result<()> {
        trace!("push_source_overlay( {overlay:?}, {id} )");

        // add new overlay to source stream
        self.get_stream_mut(id)?.push_overlay(overlay)?;

        debug!("Pushed source overlay to {id}");
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
    pub fn set_visibles(&mut self, ids: &[ID]) -> Result<()> {
        trace!("set_visibles( {ids:?} )");
        trace!("currently visible: {:?} ", self.visibles);

        // unlink all videos which will get invisible
        let hide: Vec<ID> = self
            .visibles
            .iter()
            .filter(|id| !ids.contains(id))
            .copied()
            .collect();
        for id in hide {
            self.unlink_video(&id)?;
        }

        // link all invisible videos which will get visible
        let show: Vec<ID> = ids
            .iter()
            .filter(|id| !self.visibles.contains(id))
            .copied()
            .collect();
        for id in show {
            self.link_video(&id)?;
        }

        // copy ID list of visibles
        self.visibles = ids.into();

        debug!("Set visibles to {visibles:?}", visibles = self.visibles);
        Ok(())
    }

    pub fn show(&mut self, id: &ID) -> Result<()> {
        if !self.visibles.contains(id) {
            debug!("make {id} visible");
            let mut ids = self.visibles.clone();
            ids.push(*id);
            self.set_visibles(&ids)?;
            self.invalidate();
        }
        Ok(())
    }

    /// Return `true`, if stream is currently visible
    pub fn is_visible(&self, id: &ID) -> bool {
        self.visibles.contains(id)
    }

    /// Return `true`, if stream currently provides video
    pub fn has_video(&self, id: &ID) -> Result<bool> {
        Ok(self.get_stream(id)?.status.has_video)
    }

    /// Set status of a stream.
    ///
    /// # Arguments
    ///
    /// - `id`: Describes which stream shall be updated.
    /// - `new_status`: New status to override.
    ///
    pub fn set_status(&mut self, id: &ID, new_status: StreamStatus) -> Result<StreamStatus> {
        trace!("set_status( {id}, {new_status} )");

        // get old stream's status
        let old_status = self
            .streams
            .get(id)
            .ok_or_else(|| anyhow!("given stream id ({id}) cannot be found"))?
            .status
            .clone();

        // unlink stream's video/audio  from rest of the pipeline according to new status
        match self.get_stream(id)?.audio_link_status {
            LinkStatus::None => panic!("set_status failed on uninitialized audio stream ({id})"),
            LinkStatus::Unlinked(_) => {
                if new_status.has_audio && !old_status.has_audio {
                    self.link_audio(id)?;
                }
            }
            LinkStatus::Linked(_) => {
                if !new_status.has_audio && old_status.has_audio {
                    self.unlink_audio(id)?;
                }
            }
        }
        match self.get_stream(id)?.video_link_status {
            LinkStatus::None => panic!("set_status failed on uninitialized video stream ({id})"),
            LinkStatus::Unlinked(_) => {
                if new_status.has_video && !old_status.has_video {
                    self.link_video(id)?;
                }
            }
            LinkStatus::Linked(_) => {
                if !new_status.has_video && old_status.has_video {
                    self.unlink_video(id)?;
                    if let Some(pos) = self.visibles.iter().position(|i| i == id) {
                        self.visibles.remove(pos);
                    }
                }
            }
        }

        // set stream's new status
        self.get_stream_mut(id)?.status = new_status;

        Ok(old_status)
    }

    /// Access the mixer's mutable streams.
    fn get_stream_mut(&mut self, id: &ID) -> Result<&mut Stream<SRC>> {
        self.streams
            .get_mut(id)
            .ok_or_else(|| anyhow!("given stream id ({id}) cannot be found"))
    }

    /// Access the mixer's streams.
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
    /// - `details`: Details of graph.
    ///
    pub fn dot(&self, filename_without_extension: &str, params: &debug::Params) {
        debug::dot_ext(&self.pipeline, filename_without_extension, params)
    }

    /// Re-layout the current compositor scene.
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

        // layout all video streams
        for (n, id) in self.visibles.iter().enumerate() {
            let stream = self.get_stream(id)?;
            // find all linked videos
            if let LinkStatus::Linked(valve) = &stream.video_link_status {
                // get linked mixer sink
                let valve_src = valve
                    .static_pad("src")
                    .ok_or_else(|| anyhow!("src pad of valve not found ({id})"))?;
                let mixer_sink = valve_src
                    .peer()
                    .ok_or_else(|| anyhow!("mixer sink at valve not found ({id})"))?;
                // layout current stream at this sink
                let view = layout.view(n);
                mixer_sink.set_properties(&[
                    ("xpos", &(view.pos.x as i32).to_value()),
                    ("ypos", &(view.pos.y as i32).to_value()),
                    ("width", &(view.size.width as i32).to_value()),
                    ("height", &(view.size.height as i32).to_value()),
                    ("alpha", &(view.alpha).to_value()),
                ]);
            }
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

    /// Link stream's audio source to audio mixer.
    ///
    /// # Arguments
    ///
    /// - `id`: Describes which stream's audio shall be linked.
    ///
    fn link_audio(&mut self, id: &ID) -> Result<()> {
        trace!("link_audio({id})");

        // prefetch audio mixer
        let audio_mixer = self.audio_mixer.clone();

        // find stream in our list
        let stream = self.get_stream_mut(id)?;

        // check audio link status
        match &stream.audio_link_status {
            LinkStatus::None => panic!("Uninitialized audio source ({id})"),
            LinkStatus::Unlinked(valve) => {
                // unlink source from fakesink, link source to mixer and remove fakesink
                dynamic::link_source(valve, &audio_mixer)?;

                // update audio link status
                stream.audio_link_status = LinkStatus::Linked(valve.clone());

                debug!("Linked audio of {id} to audiomixer");
            }
            LinkStatus::Linked(_) => {
                warn!("trying to link stream {id} to compositor when it is already linked");
            }
        }

        Ok(())
    }

    /// Unlink stream's audio from the audiomixer.
    ///
    /// # Arguments
    ///
    /// - `id`: Describes which stream's audio shall be unlinked.
    ///
    fn unlink_audio(&mut self, id: &ID) -> Result<()> {
        trace!("unlink_audio({id})");

        // prefetch audio mixer
        let audio_mixer = self.audio_mixer.clone();

        // find stream in our list
        let stream = self.get_stream_mut(id)?;

        // check audio link status
        match &stream.audio_link_status {
            LinkStatus::None => panic!("Uninitialized audio source ({id})"),
            LinkStatus::Unlinked(_) => {
                warn!("trying to link stream {id} to fakesink when it is already linked");
            }
            LinkStatus::Linked(valve) => {
                dynamic::unlink_source(valve, &audio_mixer)?;
                stream.audio_link_status = LinkStatus::Unlinked(valve.clone());

                debug!("Linked audio of {id} to fakesink");
            }
        }
        Ok(())
    }

    /// Link stream's source to video compositor.
    ///
    /// # Arguments
    ///
    /// - `id`: Describes which stream's video shall be linked.
    ///
    fn link_video(&mut self, id: &ID) -> Result<()> {
        trace!("link_video({id})");

        // prefetch compositor
        let compositor = self.compositor.clone();

        // find stream in our list
        let stream = self.get_stream_mut(id)?;

        // check video link status
        match &stream.video_link_status {
            LinkStatus::None => panic!("Uninitialized video source ({id})"),
            LinkStatus::Unlinked(valve) => {
                // unlink source from fakesink, link source to compositor and remove fakesink
                let sink = dynamic::link_source(valve, &compositor)?;

                // set sizing policy at compositor sink
                sink.set_property_from_str("sizing-policy", "keep-aspect-ratio");
                // initially hide video until layout shows it
                sink.set_property_from_str("alpha", "0");

                // update video link status
                stream.video_link_status = LinkStatus::Linked(valve.clone());

                // request re-layout
                self.invalidate();

                debug!("Linked video of {id} to compositor");
            }
            LinkStatus::Linked(_) => {
                warn!("trying to link stream {id} to compositor when it is already linked");
            }
        }

        Ok(())
    }

    /// Link stream's video source to fake sink (while it's invisible).
    ///
    /// # Arguments
    ///
    /// - `id`: Describes which stream's video shall be unlinked.
    ///
    fn unlink_video(&mut self, id: &ID) -> Result<()> {
        trace!("unlink_video({id})");

        // prefetch compositor
        let compositor = self.compositor.clone();

        // find stream in our list
        let stream = self.get_stream_mut(id)?;

        // check video link status
        match &stream.video_link_status {
            LinkStatus::None => panic!("Uninitialized video source ({id})"),
            LinkStatus::Unlinked(_) => {
                warn!("trying to link stream {id} to fakesink when it is already linked");
            }
            LinkStatus::Linked(valve) => {
                dynamic::unlink_source(valve, &compositor)?;
                stream.video_link_status = LinkStatus::Unlinked(valve.clone());

                // request re-layout
                self.invalidate();

                debug!("Linked video of {id} to fakesink");
            }
        }
        Ok(())
    }
}

impl<SRC, ID> Drop for Mixer<SRC, ID>
where
    SRC: Source,
    SRC::Parameters: Debug,
    ID: Eq + Ord + Hash + Copy + Debug + Display,
{
    /// halt pipeline (can not be played again)
    fn drop(&mut self) {
        debug!("Dropping mixer...");

        debug!("Sending EOS...");

        // ensure playing
        assert_eq!(self.pipeline.current_state(), gst::State::Playing);

        // send expected EOS
        self.expect_eos.store(true, Ordering::SeqCst);
        self.pipeline.send_event(gst::event::Eos::new());
        debug::debug_dot(&self.pipeline, "EOS");

        // wait until the bus reader thread got the EOS and finishes
        if let Some(is_reading_bus) = &self.is_reading_bus {
            if is_reading_bus.recv_timeout(BUS_EOS_TIMEOUT).is_err() {
                error!("Could not stop reading pipeline bus");
            }
        }

        debug!("Stop sink...");
        // call sink to prepare for dropping pipeline
        self.output.on_exit(&self.pipeline);

        debug!("Nulling pipeline...");
        self.pipeline
            .set_state(gst::State::Null)
            .expect("Unable to set the pipeline to the `Null` state");

        debug!("Exited mixer.");
    }
}
