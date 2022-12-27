// sub-modules
mod overlay;
mod sink;
mod source;
mod stream;
mod talk;

// forward useful sub-module stuff as public
pub use super::overlays::Overlay;
pub use overlay::OverlayTrait;
pub use sink::Sink;
pub use source::Source;
pub use stream::{Stream, StreamStatus};
pub use talk::{SpeakerMode, Talk};

// what else we need from this lib
use crate::{Error, Layout, Size};
use stream::VideoLinkStatus;

// what we need from external libraries
use core::{fmt::Debug, hash::Hash, mem::replace};
use gst::prelude::*;
use std::collections::HashMap;

/// Mixer managing the GStreamer pipeline using the given layout and source type
/// # Types
/// - `L`: Layout to use to compose output picture.
/// - `SRC`: Source type to use when adding streams.
/// - `SINK`: Sink type to use for output.
pub struct Mixer<SRC, SINK, ID>
where
    SRC: Source,
    SINK: Sink,
    ID: Eq + Ord + Hash + Copy + Debug,
{
    /// GStreamer element which composes the output video out of the source videos.
    compositor: gst::Element,
    /// GStreamer element which composes the output audio out of the source audios.
    audio_mixer: gst::Element,
    /// Number of currently visible streams.
    visibles: Vec<ID>,
    /// The mixer GStreamer pipeline.
    pipeline: gst::Pipeline,
    /// pad on which the first (lowest z-order) overlay's sink pad has to be attached to
    overlay_src: gst::Pad,
    /// pad on which the last (highest z-order) overlay's src pad has to be attached to
    overlay_sink: gst::Pad,
    /// on top overlays
    overlays: Vec<Overlay>,
    /// Current streams.
    streams: HashMap<ID, Stream<SRC>>,
    /// Holds the output sink.
    output: SINK,
    /// over all generated output resolution
    output_resolution: Size,
}

impl<SRC, SINK, ID> Mixer<SRC, SINK, ID>
where
    SRC: Source,
    SINK: Sink,
    ID: Eq + Ord + Hash + Copy + Debug,
{
    /// Create a new mixer and setup the initial GStreamer pipeline with the given type of sink.
    /// # Arguments
    /// - `resolution`: Output video resolution.
    /// - `sink_params`: Parameters to create the output sink.
    pub fn new(resolution: Size, sink_params: SINK::Parameters) -> Result<Self, Error<ID>> {
        // get width/height
        let width = resolution.width;
        let height = resolution.height;
        trace!(
            "Output video resolution (WxH): {width}x{height} = {:2}",
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
        let output = SINK::new(&pipeline, sink_params);

        // connect output pads to output sinks
        video_output_pad
            .link(&output.video_sink_pad())
            .expect("failed to link output pad to video output sink");
        audio_output_pad
            .link(&output.audio_sink_pad())
            .expect("failed to link output pad to audio output sink");

        let overlay_src = compositor
            .static_pad("src")
            .expect("failed to get src pad from compositor");
        let overlay_sink = video_out
            .static_pad("sink")
            .expect("failed to get src pad from video_out ");

        Ok(Mixer {
            // remember all those elements and pads
            compositor,
            audio_mixer,
            visibles: Vec::new(),
            overlay_src,
            overlay_sink,
            overlays: Vec::new(),
            pipeline,
            streams: HashMap::new(),
            output,
            output_resolution: resolution,
        })
    }
    /// Add a new stream to the mixer.
    /// # Arguments
    /// - `id`: Unique identifier of the stream.
    /// - `params`: Source specific parameters.
    pub fn add_stream(
        &mut self,
        id: ID,
        display_name: String,
        params: SRC::Parameters,
    ) -> Result<(), Error<ID>> {
        debug!("add stream '{display_name}' ({id:?})");
        // check preconditions
        if self.pipeline.current_state() == gst::State::Playing {
            return Err(Error::PlayingPipelineForbidden);
        }
        if self.streams.contains_key(&id) {
            return Err(Error::IdDoublet(id));
        }
        // add new stream
        let stream = Stream::new(
            &self.pipeline,
            &self.output_resolution,
            display_name,
            params,
        );
        self.streams.insert(id, stream);

        // link new stream
        self.link_audio(id)?;
        self.link_video_to_fakesink(id)?;

        Ok(())
    }

    pub fn state(&self) -> gst::State {
        self.pipeline.current_state()
    }

    /// remove an once added stream from the mixer.
    /// # Arguments
    /// - `id`: Unique identifier of the stream.
    pub fn remove_stream(&mut self, remove_id: ID) -> Result<(), Error<ID>> {
        debug!("remove stream {remove_id:?}");

        // check preconditions
        if self.pipeline.current_state() == gst::State::Playing {
            return Err(Error::PlayingPipelineForbidden);
        }

        // unlink stream from rest of the pipeline
        self.unlink_audio(remove_id)?;
        self.unlink_video(remove_id)?;

        // remove stream from stored streams
        let stream = self
            .streams
            .remove(&remove_id)
            .ok_or(Error::ParticipantNotFound(remove_id))?;

        stream.source.remove(&self.pipeline);

        Ok(())
    }

    /// push new overlay on top of output video within the pipeline
    /// # Arguments
    /// - `overlay`: new overlay to push
    pub fn push_overlay(&mut self, overlay: Overlay) -> Result<(), Error<ID>> {
        debug!("add overlay: {:?}", overlay);

        // check preconditions
        if self.pipeline.current_state() == gst::State::Playing {
            return Err(Error::PlayingPipelineForbidden);
        }

        // get compositor or last overlay source
        let last_src = match self.overlays.last() {
            Some(overlay) => overlay.src(),
            None => self.overlay_src.clone(),
        };
        // get sink the overall output goes to
        let output_sink = &self.overlay_sink;

        // add new element to pipeline
        self.pipeline
            .add(overlay.element())
            .expect("failed to add overlay element");

        // unlink last overlay (or compositor) source pad from output sink
        if let Some(last) = output_sink.peer() {
            last.unlink(output_sink)
                .expect("failed to unlink last overlay element or compositor from output sink");
            // link previous overlay (or compositor) source pad to the new overlay's sink
            last_src
                .link(&overlay.sink())
                .expect("failed to link previous src to new overlay's sink");
        }
        // link new overlay's source pad to output sink
        overlay
            .src()
            .link(output_sink)
            .expect("failed to link new overlay source pad to output sink");

        // remember this overlay
        self.overlays.push(overlay);

        Ok(())
    }

    /// Select the streams which are visible.
    /// All previously visible streams get invisible if they are not in the list.
    /// See set_speaker() for further info about how the order will be interpreted.
    /// # Arguments
    /// - `ids`: List of identifiers of streams which shall get visible
    pub fn set_visibles(&mut self, ids: &[ID]) -> Result<(), Error<ID>> {
        debug!("set visibles: {:?} -> {:?}", self.visibles, ids);

        // check preconditions
        if self.pipeline.current_state() == gst::State::Playing {
            return Err(Error::PlayingPipelineForbidden);
        }

        //    if let Some(max_visible) = self.max_visible {
        //        // check if given list exceeds maximum length
        //        if ids.len() > max_visible {
        //            return Err(Error::TooManyVisibles);
        //        }
        //    }

        // Unlink all participants
        for id in self.visibles.clone() {
            self.link_video_to_fakesink(id)?;
        }

        // Link all given streams
        for id in ids {
            self.link_video_to_compositor(*id)?;
        }

        // copy ID list of visibles
        self.visibles = ids.into();

        Ok(())
    }

    /*
       /// Sets the current speaker
       /// Visualization of the current speaker depends on the layout's speaker mode
       /// # Arguments
       /// - `id`: ID of the stream to mark as speaker
       /// # Speaker modes
       /// Depending on the layout::SpeakerMode set in the layout the speaker might be moved into or within the visibles.
       pub fn set_speaker(&mut self, speaker_id: Option<ID>) -> Result<(), Error<ID>> {
           debug!("set speaker {:?}...", speaker_id);

           if let Some(speaker_id) = &speaker_id {
               let mut visibles = self.visibles.clone();

               // check if speaker is stream
               if !self.streams.contains_key(speaker_id) {
                   error!("speaker must be a stream");
               }
               use layout::*;
               match self.layout.speaker_mode() {
                   SpeakerMode::FirstShift => {
                       // check if speaker is in visibles
                       match visibles.iter().position(|id| id == speaker_id) {
                           Some(pos) => {
                               trace!("remove visible at {pos}");
                               // remove speaker from visibles
                               visibles.remove(pos);
                           }
                           None => {
                               if let Some(max_visible) = self.max_visible {
                                   // remove last visible if visibles are filled completely
                                   if visibles.len() == max_visible {
                                       trace!("remove last visible");
                                       visibles.pop();
                                   }
                               }
                           }
                       }
                       trace!("insert speaker {:?} at 0", *speaker_id);
                       // insert speaker at first
                       visibles.insert(0, *speaker_id);
                   }
                   SpeakerMode::FirstSwap => {
                       // check if speaker is in visibles
                       match visibles.iter().position(|id| id == speaker_id) {
                           Some(pos) => {
                               trace!("swap visible 0 and {pos}");
                               // swap with previous speaker
                               visibles.swap(0, pos);
                           }
                           None => {
                               if let Some(max_visible) = self.max_visible {
                                   // remove last visible if visibles are filled completely
                                   if visibles.len() == max_visible {
                                       trace!("remove last visible");
                                       visibles.pop();
                                   }
                               }
                               // insert speaker at first
                               trace!("insert speaker {:?} at 0", *speaker_id);
                               visibles.insert(0, *speaker_id);
                           }
                       }
                   }
                   _ => (),
               }
               self.speaker = Some(*speaker_id);
               self.set_visibles(&visibles)?;
               self.layout()?;
           }
           Ok(())
       }
    */
    /// set status of a stream
    pub fn set_status(&mut self, id: ID, new_status: StreamStatus) -> Result<(), Error<ID>> {
        debug!("set stream {id:?} status to {new_status:?}");

        // check preconditions
        if self.pipeline.current_state() == gst::State::Playing {
            return Err(Error::PlayingPipelineForbidden);
        }

        // get old stream's status
        let old_status = self
            .streams
            .get(&id)
            .ok_or(Error::ParticipantNotFound(id))?
            .status
            .clone();

        // unlink stream's video/audio  from rest of the pipeline according to new status
        if !new_status.has_audio && old_status.has_audio {
            self.unlink_audio(id)?;
        } else if new_status.has_audio && !old_status.has_audio {
            self.link_audio(id)?;
        }
        if !new_status.has_video && old_status.has_video {
            self.unlink_video(id)?;
        } else if new_status.has_video && !old_status.has_video {
            self.link_video_to_fakesink(id)?;
        }

        // set stream's new status
        self.streams
            .get_mut(&id)
            .ok_or(Error::ParticipantNotFound(id))?
            .status = new_status;

        /*
               self.set_speaker(self.speaker)?;
        */
        Ok(())
    }

    /// start playing of pipeline
    pub fn play(&mut self) {
        debug!("play pipeline");
        self.pipeline
            .set_state(gst::State::Playing)
            .expect("failed to set pipeline state to playing");
        std::thread::sleep(std::time::Duration::from_millis(100));
        self.output.on_play();
    }

    /// pause playing of pipeline
    pub fn pause(&mut self) {
        debug!("pause pipeline");
        self.pipeline
            .set_state(gst::State::Paused)
            .expect("failed to set pipeline state to paused");
        std::thread::sleep(std::time::Duration::from_millis(100));
        self.output.on_pause();
    }

    /// generate DOT file of the current pipeline
    pub fn generate_dot_file(
        &self,
        filename_without_extension: &str,
        details: gst::DebugGraphDetails,
    ) {
        if let Ok(path) = std::env::var("GST_DEBUG_DUMP_DOT_DIR") {
            info!(
                "writing DOT file `{}/{filename_without_extension}.dot`...",
                path
            );
            gst::debug_bin_to_dot_file(&self.pipeline, details, filename_without_extension);
        } else {
            error!("can not write DOT file. You need to set GST_DEBUG_DUMP_DOT_DIR in environment to a absolute path");
        }
    }

    /// Re-layout the current compositor scene.
    pub fn layout<L>(&self) -> Result<(), Error<ID>>
    where
        L: Layout,
    {
        // check preconditions
        if self.pipeline.current_state() == gst::State::Playing {
            return Err(Error::PlayingPipelineForbidden);
        }

        // initialize the layout with mixer setup
        let layout = L::new(self.visibles.len(), self.output_resolution);

        // configure compositor sink pads (which might be connected to the streams' sources)
        for (n, pad) in self.compositor.sink_pads()[1..].iter().enumerate() {
            self.layout_stream(&layout.view(n), pad);
        }

        Ok(())
    }

    /// Layout an GstBaseTextOverlay derivate.
    fn layout_stream(&self, view: &crate::View, pad: &gst::Pad) {
        trace!("{name}: {view:?}", name = pad.name());
        pad.set_property("xpos", view.pos.x as i32);
        pad.set_property("ypos", view.pos.y as i32);
        pad.set_property("width", view.size.width as i32);
        pad.set_property("height", view.size.height as i32);
        pad.set_property("alpha", view.alpha);
    }

    // /// Layout an GstBaseTextOverlay derivate.
    // fn layout_overlay(
    //     &self,
    //     element: &Option<gst::Element>,
    //     position: Position,
    //     alignment: Alignment,
    // ) {
    //     if let Some(element) = element {
    //         element.set_property_from_str("halignment", alignment.horizontal);
    //         element.set_property_from_str("valignment", alignment.vertical);
    //         element.set_property_from_str("line-alignment", alignment.horizontal);
    //         element.set_property_from_str("deltax", &position.x.to_string());
    //         element.set_property_from_str("deltay", &position.y.to_string());
    //     }
    // }

    /// Link stream's audio source to audio mixer.
    fn link_audio(&mut self, id: ID) -> Result<(), Error<ID>> {
        trace!("linking audio of {:?}...", id);

        let stream = self
            .streams
            .get_mut(&id)
            .ok_or(Error::ParticipantNotFound(id))?;

        let mixer_pad = self
            .audio_mixer
            .request_pad_simple("sink_%")
            .expect("Failed to request sink pad from audio mixer");

        stream
            .source
            .audio_src_pad()
            .link(&mixer_pad)
            .expect("Failed to link stream's audio source to audio mixer sink pad");

        stream.audio_mixer_pad = Some(mixer_pad);

        Ok(())
    }

    /// Unlink stream's audio from the audiomixer.
    fn unlink_audio(&mut self, id: ID) -> Result<(), Error<ID>> {
        trace!("unlinking audio of {id:?}...");

        let stream = self
            .streams
            .get_mut(&id)
            .ok_or(Error::ParticipantNotFound(id))?;

        if let Some(pad) = stream.audio_mixer_pad.take() {
            stream
                .source
                .audio_src_pad()
                .unlink(&pad)
                .expect("Failed to unlink stream's audio source from audio mixer sink pad");
        }

        trace!("unlinked audio of {id:?}...");

        Ok(())
    }

    /// Link stream's video source to fake sink (while it's invisible).
    fn link_video_to_fakesink(&mut self, id: ID) -> Result<(), Error<ID>> {
        trace!("linking video of {id:?} to fakesink...");

        let stream = self
            .streams
            .get_mut(&id)
            .ok_or(Error::ParticipantNotFound(id))?;

        match &stream.video_link_status {
            VideoLinkStatus::None => {}
            VideoLinkStatus::Fakesink(_) => {
                warn!("trying to link stream {id:?} to fakesink when it is already linked");
                return Ok(());
            }
            VideoLinkStatus::Compositor(pad) => {
                stream
                    .source
                    .video_src_pad()
                    .unlink(pad)
                    .expect("failed to unlink stream's video source from compositor");
                self.compositor.release_request_pad(pad);
            }
        }

        let fakesink = gst::ElementFactory::make_with_name("fakesink", None)
            .expect("failed to create new fake sink");
        self.pipeline
            .add(&fakesink)
            .expect("failed to add fakesink to pipeline");
        stream
            .source
            .video_src_pad()
            .link(
                &fakesink
                    .static_pad("sink")
                    .expect("failed to get static sink pad from fake sink"),
            )
            .expect("failed to link stream's video source to fake sink");
        stream.video_link_status = VideoLinkStatus::Fakesink(fakesink);

        Ok(())
    }

    /// Link stream's source to video compositor.
    fn link_video_to_compositor(&mut self, id: ID) -> Result<(), Error<ID>> {
        trace!("linking video of {id:?} to compositor...");

        let stream = self
            .streams
            .get_mut(&id)
            .ok_or(Error::ParticipantNotFound(id))?;

        match &stream.video_link_status {
            VideoLinkStatus::None => {}
            VideoLinkStatus::Fakesink(fakesink) => {
                stream
                    .source
                    .video_src_pad()
                    .unlink(
                        &fakesink
                            .static_pad("sink")
                            .expect("failed to get static sink pad from fake sink"),
                    )
                    .expect("failed to unlink stream's video source from fake sink");
                fakesink
                    .set_state(gst::State::Null)
                    .expect("failed to set fake sink into Null state");
                self.pipeline
                    .remove(fakesink)
                    .expect("failed to remove fake sink from pipeline");
            }
            VideoLinkStatus::Compositor(_) => {
                warn!("trying to link stream {id:?} to compositor when it is already linked");
                return Ok(());
            }
        }

        trace!("creating compositor sink for stream {id:?}");
        let pad = self
            .compositor
            .request_pad_simple("sink_%u")
            .expect("cannot create sink pad");
        pad.set_property_from_str("sizing-policy", "keep-aspect-ratio");
        stream
            .source
            .video_src_pad()
            .link(&pad)
            .expect("failed to link stream's video source pad to compositor pad");
        stream.video_link_status = VideoLinkStatus::Compositor(pad);

        trace!("successfully linked video of {id:?} to compositor.");

        Ok(())
    }

    // Unlink stream's video source from compositor.
    fn unlink_video(&mut self, id: ID) -> Result<(), Error<ID>> {
        trace!("unlinking video of {id:?}...");

        let stream = self
            .streams
            .get_mut(&id)
            .ok_or(Error::ParticipantNotFound(id))?;

        match replace(&mut stream.video_link_status, VideoLinkStatus::None) {
            VideoLinkStatus::None => {}
            VideoLinkStatus::Fakesink(fakesink) => {
                stream
                    .source
                    .video_src_pad()
                    .unlink(
                        &fakesink
                            .static_pad("sink")
                            .expect("failed to get static sink pad from fake sink"),
                    )
                    .expect("failed to unlink stream's video source pad from fake sink");
                fakesink
                    .set_state(gst::State::Null)
                    .expect("failed to set fake sink to Null state");
                self.pipeline
                    .remove(&fakesink)
                    .expect("failed to remove fake sink from pipeline");
            }
            VideoLinkStatus::Compositor(pad) => {
                stream
                    .source
                    .video_src_pad()
                    .unlink(&pad)
                    .expect("failed to unlink stream's video source pad from compositor");
                self.compositor.release_request_pad(&pad);
            }
        }

        trace!("unlinked video of {id:?}...");

        Ok(())
    }
}

impl<SRC, SINK, ID> Drop for Mixer<SRC, SINK, ID>
where
    SRC: Source,
    SINK: Sink,
    ID: Eq + Ord + Hash + Copy + Debug,
{
    /// halt pipeline (can not be played again)
    fn drop(&mut self) {
        debug!("exiting mixer...");

        if self.pipeline.current_state() == gst::State::Paused {
            self.pipeline
                .set_state(gst::State::Playing)
                .expect("failed to set pipeline state to playing");
        }

        self.pause();

        for id in self.streams.keys().cloned().collect::<Vec<_>>() {
            let _ = self.remove_stream(id);
        }

        // call sink to prepare for dropping pipeline
        self.output.on_exit(&self.pipeline);

        // stop pipeline
        self.pipeline
            .set_state(gst::State::Null)
            .expect("unable to set the pipeline to the `Null` state");

        debug!("...mixer exited successfully.");
    }
}
