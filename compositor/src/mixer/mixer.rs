use crate::{error::Error, layout::*, mixer::Participant};
use gst::{
    prelude::{ElementExtManual, GObjectExtManualGst, ObjectExt},
    traits::{ElementExt, GstBinExt, GstObjectExt, PadExt},
};
use gstreamer as gst;
use std::collections::HashMap;

pub trait Source {
    fn new(pipeline: &gst::Pipeline, name: &str, pattern: &str, resolution: &Size) -> Self;
    fn video_src_element(&self) -> &gst::Element;
    fn video_src_pad(&self) -> &gst::Pad;
    fn video_sink_pad(&self) -> &gst::Pad;
    fn video_fake_sink(&self) -> &Option<gst::Element>;
    fn set_video_sink_pad(&mut self, sink_pad: gst::Pad);
    fn set_video_fake_sink(&mut self, fake_sink: Option<gst::Element>);
    fn audio_src_element(&self) -> &gst::Element;
    fn audio_src_pad(&self) -> &gst::Pad;
    fn audio_sink_pad(&self) -> &gst::Pad;
    fn audio_fake_sink(&self) -> &Option<gst::Element>;
    fn set_audio_sink_pad(&mut self, sink_pad: gst::Pad);
    fn set_audio_fake_sink(&mut self, fake_sink: Option<gst::Element>);
}

pub trait Sink {
    fn new(pipeline: &gst::Pipeline, _resolution: &Size) -> Self;
    fn video_sink_pad(&self) -> &gst::Pad;
    fn audio_sink_pad(&self) -> &gst::Pad;
}

#[derive(Clone)]
pub struct Mixer<L, SRC>
where
    L: Layout,
    SRC: Source,
{
    pub compositor: gst::Element,
    pub audio_mixer: gst::Element,
    resolution: Size,
    pub max_participants: usize,
    pub max_visibles: usize,
    pub visibles: usize,
    clock: Option<gst::Element>,
    title: Option<gst::Element>,
    speaking: Option<gst::Element>,
    pipeline: gst::Pipeline,
    layout: L,
    pub participants: HashMap<String, Participant<SRC>>,
}

impl<L, SRC> Mixer<L, SRC>
where
    L: Layout,
    SRC: Source,
{
    pub fn new<SINK>(
        resolution: &Size,
        max_participants: usize,
        max_visibles: usize,
    ) -> Result<Self, Error>
    where
        SINK: Sink,
    {
        if max_participants < max_visibles {
            return Err(Error::MoreMaxVisiblesThanMaxParticipants);
        }
        let width = resolution.width;
        let height = resolution.height;
        let layout = L::new(&resolution);
        let pipeline = gst::Pipeline::new(None);

        // create output link
        let output = SINK::new(&pipeline, &resolution);

        // create video test src to get a picture when no participant is connected
        let video_background_src =
            gst::ElementFactory::make("videotestsrc", Some(&format!("video-background"))).unwrap();
        video_background_src.set_property_from_str("pattern", "black");
        video_background_src.set_property_from_str("is-live", "true");

        let video_background_queue =
            gst::ElementFactory::make("queue", Some(&format!("video-background-queue"))).unwrap();

        // create video caps setter
        let video_caps =
            gst::ElementFactory::make("capssetter", Some(&format!("video-caps"))).unwrap();
        video_caps.set_property_from_str(
            "caps",
            &format!("video/x-raw,format=RGB,width={width},height={height}",),
        );

        // create video compositor
        let compositor =
            gst::ElementFactory::make("compositor", Some(&format!("video-compositor"))).unwrap();
        compositor.set_property_from_str("ignore-inactive-pads", "true");
        for _ in 0..max_visibles + 1 {
            compositor.request_pad_simple("sink_%u").unwrap();
        }

        // create video clock overlay
        let clock_overlay =
            gst::ElementFactory::make("clockoverlay", Some(&format!("video-clock-overlay")))
                .unwrap();
        clock_overlay.set_property_from_str("font-desc", "Sans, 14");
        clock_overlay.set_property_from_str("time-format", "%x %X %Z");
        clock_overlay.set_property_from_str("xpad", "10");
        clock_overlay.set_property_from_str("ypad", "2");
        clock_overlay.set_property_from_str("color", "0xffffffff");

        // create video title text overlay
        let title_overlay =
            gst::ElementFactory::make("textoverlay", Some(&format!("video-title-overlay")))
                .unwrap();
        title_overlay.set_property_from_str("font-desc", "Sans, 16");
        title_overlay.set_property_from_str("xpad", "10");
        title_overlay.set_property_from_str("ypad", "2");
        title_overlay.set_property_from_str("color", "0xffffffff");

        // create video speaking text overlay
        let speaking_overlay =
            gst::ElementFactory::make("textoverlay", Some(&format!("video-speaking-overlay")))
                .unwrap();
        speaking_overlay.set_property_from_str("font-desc", "Sans, 16");
        speaking_overlay.set_property_from_str("xpad", "10");
        speaking_overlay.set_property_from_str("ypad", "2");
        speaking_overlay.set_property_from_str("color", "0xffffffff");

        // create video output queue
        let video_output_queue =
            gst::ElementFactory::make("queue", Some(&format!("video-output-queue"))).unwrap();
        let video_output_pad = video_output_queue.static_pad("src").unwrap();

        // add video elements to pipeline
        pipeline.add(&video_background_src).unwrap();
        pipeline.add(&video_caps).unwrap();
        pipeline.add(&video_background_queue).unwrap();
        pipeline.add(&compositor).unwrap();
        pipeline.add(&clock_overlay).unwrap();
        pipeline.add(&title_overlay).unwrap();
        pipeline.add(&speaking_overlay).unwrap();
        pipeline.add(&video_output_queue).unwrap();

        // link video elements
        video_background_src.link(&video_caps).unwrap();
        video_caps.link(&video_background_queue).unwrap();
        video_background_queue.link(&compositor).unwrap();
        compositor.link(&clock_overlay).unwrap();
        clock_overlay.link(&title_overlay).unwrap();
        title_overlay.link(&speaking_overlay).unwrap();
        speaking_overlay.link(&video_output_queue).unwrap();
        // link to output sink
        video_output_pad.link(output.video_sink_pad()).unwrap();

        // create test src to get a picture when no participant is connected
        let audio_background_src =
            gst::ElementFactory::make("audiotestsrc", Some(&format!("audio-background"))).unwrap();
        audio_background_src.set_property_from_str("is-live", "true");
        audio_background_src.set_property_from_str("volume", "0.01");

        // create audio caps setter
        let audio_caps =
            gst::ElementFactory::make("capssetter", Some(&format!("audio-caps"))).unwrap();
        audio_caps.set_property_from_str(
            "caps",
            &format!("audio/x-raw,format=S16LE,channels=2,layout=interleaved,rate=48000",),
        );

        let audio_background_queue =
            gst::ElementFactory::make("queue", Some(&format!("audio-background-queue"))).unwrap();

        // create audio mixer
        let audio_mixer =
            gst::ElementFactory::make("audiomixer", Some(&format!("audio-mixer"))).unwrap();
        audio_mixer.set_property_from_str("ignore-inactive-pads", "true");
        for _ in 0..max_participants + 1 {
            audio_mixer.request_pad_simple("sink_%u").unwrap();
        }

        // create audio output queue
        let audio_output_queue =
            gst::ElementFactory::make("queue", Some(&format!("audio-queue"))).unwrap();
        let audio_output_pad = audio_output_queue.static_pad("src").unwrap();

        // link audio elements
        pipeline.add(&audio_background_src).unwrap();
        pipeline.add(&audio_caps).unwrap();
        pipeline.add(&audio_background_queue).unwrap();
        pipeline.add(&audio_mixer).unwrap();
        pipeline.add(&audio_output_queue).unwrap();

        // link audio elements
        audio_background_src.link(&audio_caps).unwrap();
        audio_caps.link(&audio_background_queue).unwrap();
        audio_background_queue.link(&audio_mixer).unwrap();
        audio_mixer.link(&audio_output_queue).unwrap();
        // link to output sink
        audio_output_pad.link(output.audio_sink_pad()).unwrap();

        Ok(Mixer {
            // remember elements and pads for connect/disconnect and property setup
            compositor: compositor.clone(),
            audio_mixer: audio_mixer.clone(),
            resolution: resolution.clone(),
            max_participants,
            max_visibles,
            visibles: 0,
            clock: Some(clock_overlay.clone()),
            title: Some(title_overlay.clone()),
            speaking: Some(speaking_overlay.clone()),
            layout,
            pipeline: pipeline.clone(),
            participants: HashMap::new(),
        })
    }
    pub fn add_participants(&mut self, names: &[String]) -> Result<(), Error> {
        if self.pipeline.current_state() == gst::State::Playing {
            return Err(Error::PlayingPipelineForbidden);
        }
        for name in names {
            if self.participants.len() >= self.max_participants {
                return Err(Error::TooManyParticipants);
            }
            let participant = Participant::new(&self.pipeline, name, &self.resolution);
            self.participants.insert(name.to_string(), participant);
        }
        self.link_audio(&names.to_vec())?;
        Ok(())
    }
    pub fn set_visibles(&mut self, names: &[String]) -> Result<(), Error> {
        if self.pipeline.current_state() == gst::State::Playing {
            return Err(Error::PlayingPipelineForbidden);
        }
        if names.len() > self.max_visibles {
            return Err(Error::TooManyVisibles);
        }
        // unlink all participants from video compositor
        self.unlink_video(&self.participants.iter().map(|(n, _)| n.clone()).collect())?;
        // convert names to Vec<String>
        let names = &names
            .iter()
            .map(|name| name.clone())
            .collect::<Vec<String>>();
        if !names.is_empty() {
            self.link_video(names)?;
        }
        Ok(())
    }
    pub fn layout(&self) -> Result<(), Error> {
        if self.pipeline.current_state() == gst::State::Playing {
            return Err(Error::PlayingPipelineForbidden);
        }
        let count = self.visibles;
        trace!("visibles = {count}");
        self.layout_overlay(
            &self.title,
            self.layout.title_position(count),
            self.layout.title_alignment(),
        );
        self.layout_overlay(
            &self.clock,
            self.layout.clock_position(count),
            self.layout.clock_alignment(),
        );
        self.layout_overlay(
            &self.speaking,
            self.layout.speaking_position(count),
            self.layout.speaking_alignment(count),
        );
        for (n, pad) in self.compositor.sink_pads()[1..].iter().enumerate() {
            let (pos, size, alpha) = if n < count {
                (
                    self.layout.position(n, count),
                    self.layout.size(n, count),
                    1.0,
                )
            } else {
                (
                    Position { x: 0, y: 0 },
                    Size {
                        width: 0,
                        height: 0,
                    },
                    0.0,
                )
            };
            trace!(
                "{name}: xpos={xpos}, ypos={ypos}, width={width}, height={height}",
                xpos = pos.x as i32,
                ypos = pos.y as i32,
                width = size.width as i32,
                height = size.height as i32,
                name = pad.name()
            );
            pad.set_property("xpos", pos.x as i32);
            pad.set_property("ypos", pos.y as i32);
            pad.set_property("width", size.width as i32);
            pad.set_property("height", size.height as i32);
            pad.set_property("alpha", alpha);
        }
        Ok(())
    }
    /// set the 'who's speaking?' text within the mixer view if provided
    pub fn set_title(&self, text: &str) {
        if let Some(title) = &self.title {
            title.set_property("text", text);
        }
    }

    /// set the title text within the mixer view if provided
    pub fn set_speaking(&self, text: &str) {
        if let Some(speaking) = &self.speaking {
            speaking.set_property("text", text);
        }
    }
    pub fn play(&self) {
        self.pipeline.set_state(gst::State::Playing).unwrap();
        std::thread::sleep_ms(200);
    }
    pub fn pause(&self) {
        self.pipeline.set_state(gst::State::Paused).unwrap();
        std::thread::sleep_ms(200);
    }
    /// wait until mixer generates error or ends
    pub fn run(&self) {
        // wait until error or EOS
        let bus = self.pipeline.bus().unwrap();
        for msg in bus.iter_timed(gst::ClockTime::NONE) {
            use gst::MessageView;

            match msg.view() {
                MessageView::Error(err) => {
                    eprintln!(
                        "Error received from element {:?}: {}",
                        err.src().map(|s| s.path_string()),
                        err.error()
                    );
                    eprintln!("Debugging information: {:?}", err.debug());
                    break;
                }
                MessageView::Eos(..) => break,
                _ => (),
            }
        }

        // stop pipeline
        self.pipeline
            .set_state(gst::State::Null)
            .expect("Unable to set the pipeline to the `Null` state");
    }
    pub fn generate_dot_file(&self, filename_without_extension: &str) {
        if let Ok(path) = std::env::var("GST_DEBUG_DUMP_DOT_DIR") {
            info!(
                "writing DOT file `{}/{filename_without_extension}.dot`...",
                path
            );
            gst::debug_bin_to_dot_file(
                &self.pipeline,
                gst::DebugGraphDetails::ALL,
                filename_without_extension,
            );
        } else {
            warn!("can not write DOT file. You need to set GST_DEBUG_DUMP_DOT_DIR in environment to a absolute path");
        }
    }
}

impl<L, SRC> Mixer<L, SRC>
where
    L: Layout,
    SRC: Source,
{
    fn layout_overlay(
        &self,
        element: &Option<gst::Element>,
        position: Position,
        alignment: Alignment,
    ) {
        if let Some(element) = element {
            element.set_property_from_str("halignment", alignment.horizontal);
            element.set_property_from_str("valignment", alignment.vertical);
            element.set_property_from_str("line-alignment", alignment.horizontal);
            element.set_property_from_str("deltax", &position.x.to_string());
            element.set_property_from_str("deltay", &position.y.to_string());
        }
    }
    fn link_audio(&mut self, names: &Vec<String>) -> Result<(), Error> {
        trace!("linking audio of {:?}...", names);
        let audiomixer_sink_pads = self.audio_mixer.sink_pads();
        for (n, name) in names.iter().enumerate() {
            if let Some(participant) = self.participants.get_mut(name) {
                let source = &mut participant.source;
                // check if not already linked to compositor
                if let Some(fake_sink) = source.audio_fake_sink() {
                    if source
                        .audio_src_pad()
                        .unlink(source.audio_sink_pad())
                        .is_ok()
                    {
                        trace!("unlinking audio fake sink pad {name}...");
                        // halt fake sink
                        fake_sink.set_state(gst::State::Null).unwrap();
                        // remove fake sink from pipeline
                        self.pipeline.remove(fake_sink).unwrap();
                        // link source with compositor
                        source
                            .audio_src_pad()
                            .link(&audiomixer_sink_pads[n + 1])
                            .unwrap();
                    }

                    // remove fake sink from compositor to signal that we have unlinked it
                    source.set_audio_fake_sink(None);
                    // save new compositor sink pad
                    source.set_audio_sink_pad(audiomixer_sink_pads[n + 1].clone());
                    trace!(
                        "linked audio of {name} successfully",
                        name = participant.name
                    );
                }
            } else {
                return Err(Error::ParticipantNotFound(name.clone()));
            }
        }
        Ok(())
    }
    #[allow(dead_code)]
    fn unlink_audio(&mut self, names: &Vec<String>) -> Result<(), Error> {
        trace!("unlinking audio of {:?}...", names);
        for (_, name) in names.iter().enumerate() {
            if let Some(participant) = self.participants.get_mut(name) {
                let source = &mut participant.source;
                // check if not already linked to fake sink
                if source.audio_fake_sink().is_none() {
                    // create fake sink
                    trace!("creating new audio fake sink for {name}...");
                    let fake_sink = gst::ElementFactory::make(
                        "fakesink",
                        Some(&format!("audio-fakesink-{name}", name = participant.name)),
                    )
                    .unwrap();
                    fake_sink.set_property_from_str("sync", "true");
                    if let Some(peer) = source.audio_src_pad().peer() {
                        trace!(
                            "unlinking audio mixer {sink} from {source}...",
                            sink = peer.name(),
                            source = name
                        );
                        source.audio_src_pad().unlink(&peer).unwrap();

                        trace!("add audio fake sink of {name} to pipeline...");
                        self.pipeline.add(&fake_sink).unwrap();
                        trace!("link audio fake sink pad for {name}...");
                        source
                            .audio_src_pad()
                            .link(&fake_sink.static_pad("sink").unwrap())
                            .unwrap();
                    }
                    source.set_audio_sink_pad(fake_sink.static_pad("sink").unwrap());
                    source.set_audio_fake_sink(Some(fake_sink));
                    self.visibles = self.visibles - 1;
                    trace!(
                        "unlinked audio of {name} successfully",
                        name = participant.name
                    );
                }
            } else {
                return Err(Error::ParticipantNotFound(name.clone()));
            }
        }
        Ok(())
    }
    fn link_video(&mut self, names: &Vec<String>) -> Result<(), Error> {
        trace!("linking video of {:?}...", names);
        let compositor_sink_pads = self.compositor.sink_pads();
        for (n, name) in names.iter().enumerate() {
            if let Some(participant) = self.participants.get_mut(name) {
                let source = &mut participant.source;
                // check if not already linked to compositor
                if let Some(fake_sink) = source.video_fake_sink() {
                    if source
                        .video_src_pad()
                        .unlink(source.video_sink_pad())
                        .is_ok()
                    {
                        trace!("unlinking video fake sink pad of {name}...");
                        // halt fake sink
                        fake_sink.set_state(gst::State::Null).unwrap();
                        // remove fake sink from pipeline
                        self.pipeline.remove(fake_sink).unwrap();
                        // link source with compositor
                        source
                            .video_src_pad()
                            .link(&compositor_sink_pads[n + 1])
                            .unwrap();
                    }

                    // remove fake sink from compositor to signal that we have unlinked it
                    source.set_video_fake_sink(None);
                    // save new compositor sink pad
                    source.set_video_sink_pad(compositor_sink_pads[n + 1].clone());
                    self.visibles = self.visibles + 1;
                    trace!(
                        "linked video of {name} successfully",
                        name = participant.name
                    );
                }
            } else {
                return Err(Error::ParticipantNotFound(name.clone()));
            }
        }
        Ok(())
    }
    fn unlink_video(&mut self, names: &Vec<String>) -> Result<(), Error> {
        trace!("unlinking video of {:?}...", names);
        for (_, name) in names.iter().enumerate() {
            if let Some(participant) = self.participants.get_mut(name) {
                let source = &mut participant.source;
                // check if not already linked to fake sink
                if source.video_fake_sink().is_none() {
                    // create fake sink
                    trace!("creating new video fake sink for {name}...");
                    let fake_sink = gst::ElementFactory::make(
                        "fakesink",
                        Some(&format!("video-fakesink-{name}", name = participant.name)),
                    )
                    .unwrap();
                    fake_sink.set_property_from_str("sync", "true");
                    if let Some(peer) = source.video_src_pad().peer() {
                        trace!(
                            "unlinking video compositor {sink} from {source}...",
                            sink = peer.name(),
                            source = name
                        );
                        source.video_src_pad().unlink(&peer).unwrap();

                        trace!("add video fake sink of {name} to pipeline...");
                        self.pipeline.add(&fake_sink).unwrap();
                        trace!("link video source of {name} fake sink pad...");
                        source
                            .video_src_pad()
                            .link(&fake_sink.static_pad("sink").unwrap())
                            .unwrap();
                    }
                    source.set_video_sink_pad(fake_sink.static_pad("sink").unwrap());
                    source.set_video_fake_sink(Some(fake_sink));
                    self.visibles = self.visibles - 1;
                    trace!(
                        "unlinked video of {name} successfully",
                        name = participant.name
                    );
                }
            } else {
                return Err(Error::ParticipantNotFound(name.clone()));
            }
        }
        Ok(())
    }
}
