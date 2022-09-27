use std::collections::HashMap;

use super::display_sink::DisplaySink;

use crate::{error::Error, layout::*, mixer::Participant};
use gst::{
    prelude::{ElementExtManual, GObjectExtManualGst, ObjectExt},
    traits::{ElementExt, GstBinExt, GstObjectExt, PadExt},
};
use gst_sdp::gst::PadExtManual;
use gstreamer as gst;

pub struct Mixer<L>
where
    L: Layout,
{
    elements: Vec<gst::Element>,
    pub compositor: gst::Element,
    resolution: Size,
    pub max_participants: usize,
    pub visibles: usize,
    clock: Option<gst::Element>,
    title: Option<gst::Element>,
    speaking: Option<gst::Element>,
    output_pad: gst::Pad,
    pipeline: gst::Pipeline,
    layout: L,
    pub participants: HashMap<String, Participant>,
}

impl<L> Mixer<L>
where
    L: Layout,
{
    pub fn new(
        pipeline: &gst::Pipeline,
        resolution: &Size,
        max_participants: usize,
        layout: L,
        _test_sink: bool,
    ) -> Mixer<L> {
        let width = resolution.width;
        let height = resolution.height;

        // create test src to get a picture when no participant is connected
        let background_src =
            gst::ElementFactory::make("videotestsrc", Some(&format!("compositor-background")))
                .unwrap();
        background_src.set_property_from_str("pattern", "black");
        background_src.set_property_from_str("is-live", "true");

        let background_queue =
            gst::ElementFactory::make("queue", Some(&format!("background-output"))).unwrap();

        // create compositor
        let compositor =
            gst::ElementFactory::make("compositor", Some(&format!("compositor"))).unwrap();
        compositor.set_property_from_str("background", "checker");
        compositor.set_property_from_str("ignore-inactive-pads", "true");
        for _ in 0..max_participants {
            compositor.request_pad_simple("sink_%u").unwrap();
        }
        // create clock overlay
        let clock_overlay =
            gst::ElementFactory::make("clockoverlay", Some(&format!("compositor-clock"))).unwrap();
        clock_overlay.set_property_from_str("font-desc", "Sans, 14");
        clock_overlay.set_property_from_str("time-format", "%x %X %Z");
        clock_overlay.set_property_from_str("xpad", "10");
        clock_overlay.set_property_from_str("ypad", "2");
        clock_overlay.set_property_from_str("color", "0xffffffff");

        // create title overlay
        let title_overlay =
            gst::ElementFactory::make("textoverlay", Some(&format!("compositor-title"))).unwrap();
        title_overlay.set_property_from_str("font-desc", "Sans, 16");
        title_overlay.set_property_from_str("xpad", "10");
        title_overlay.set_property_from_str("ypad", "2");
        title_overlay.set_property_from_str("color", "0xffffffff");

        // create speaking overlay
        let speaking_overlay =
            gst::ElementFactory::make("textoverlay", Some(&format!("compositor-speaking")))
                .unwrap();
        speaking_overlay.set_property_from_str("font-desc", "Sans, 16");
        speaking_overlay.set_property_from_str("xpad", "10");
        speaking_overlay.set_property_from_str("ypad", "2");
        speaking_overlay.set_property_from_str("color", "0xffffffff");

        // create caps setter
        let caps =
            gst::ElementFactory::make("capssetter", Some(&format!("compositor-caps"))).unwrap();
        caps.set_property_from_str(
            "caps",
            &format!("video/x-raw,format=RGB,width={width},height={height}",),
        );

        let output_queue =
            gst::ElementFactory::make("queue", Some(&format!("compositor-output"))).unwrap();

        // add elements to pipeline
        pipeline.add(&background_src).unwrap();
        pipeline.add(&caps).unwrap();
        pipeline.add(&background_queue).unwrap();
        pipeline.add(&compositor).unwrap();
        pipeline.add(&clock_overlay).unwrap();
        pipeline.add(&title_overlay).unwrap();
        pipeline.add(&speaking_overlay).unwrap();
        pipeline.add(&output_queue).unwrap();

        // link elements
        background_src.link(&caps).unwrap();
        caps.link(&background_queue).unwrap();
        background_queue.link(&compositor).unwrap();
        compositor.link(&clock_overlay).unwrap();
        clock_overlay.link(&title_overlay).unwrap();
        title_overlay.link(&speaking_overlay).unwrap();
        speaking_overlay.link(&output_queue).unwrap();

        Mixer {
            // remember elements for deletion
            elements: vec![
                background_src.clone(),
                compositor.clone(),
                clock_overlay.clone(),
                title_overlay.clone(),
                speaking_overlay.clone(),
                caps.clone(),
                output_queue.clone(),
            ],
            // remember elements and pads for connect/disconnect and property setup
            compositor: compositor.clone(),
            resolution: resolution.clone(),
            max_participants,
            visibles: 0,
            clock: Some(clock_overlay.clone()),
            title: Some(title_overlay.clone()),
            speaking: Some(speaking_overlay.clone()),
            output_pad: output_queue.static_pad("src").unwrap(),
            layout,
            pipeline: pipeline.clone(),
            participants: HashMap::new(),
        }
    }
    pub fn set_viewable(&mut self, names: &[String]) {
        self.unlink(
            &self
                .participants
                .keys()
                .map(|name| name.clone())
                .collect::<Vec<String>>(),
        )
        .unwrap();
        if !names.is_empty() {
            self.link(
                &names
                    .iter()
                    .map(|name| name.clone())
                    .collect::<Vec<String>>(),
            )
            .unwrap();
        }
    }
    pub fn link_display_sink(&self, sink: &DisplaySink) {
        self.output_pad.link(sink.sink_pad()).unwrap();
    }
    pub fn layout(&self) {
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
    }
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
    pub fn link(&mut self, names: &Vec<String>) -> Result<(), Error> {
        if (self.compositor.num_sink_pads() - 1) as usize >= self.max_participants {
            return Err(Error::TooManyParticipants);
        }
        trace!("linking {:?}...", names);
        let compositor_sink_pads = self.compositor.sink_pads();
        for (n, name) in names.iter().enumerate() {
            if let Some(participant) = self.participants.get_mut(&name.clone()) {
                // check if not already linked to compositor
                if let Some(fake_sink) = &participant.video_fake_sink {
                    if participant
                        .video_src_pad
                        .unlink(&participant.video_sink_pad)
                        .is_ok()
                    {
                        trace!(
                            "unlinking video fake sink pad {sink} from {source}...",
                            sink = participant.video_sink_pad.name(),
                            source = name
                        );
                        // halt fake sink
                        fake_sink.set_state(gst::State::Null).unwrap();
                        // remove fake sink from pipeline
                        self.pipeline.remove(fake_sink).unwrap();
                        // link source with compositor
                        participant
                            .video_src_pad
                            .link(&compositor_sink_pads[n + 1])
                            .unwrap();
                    }

                    // remove fake sink from compositor to signal that we have unlinked it
                    participant.video_fake_sink = None;
                    // save new compositor sink pad
                    participant.video_sink_pad = compositor_sink_pads[n + 1].clone();
                    self.visibles = self.visibles + 1;
                    trace!("linked {name} successfully", name = participant.name);
                }
            } else {
                return Err(Error::ParticipantNotFound(name.clone()));
            }
        }
        Ok(())
    }
    pub fn unlink(&mut self, names: &Vec<String>) -> Result<(), Error> {
        trace!("unlinking {:?}...", names);
        for (_, name) in names.iter().enumerate() {
            if let Some(participant) = self.participants.get_mut(&name.clone()) {
                // check if not already linked to fake sink
                if participant.video_fake_sink.is_none() {
                    // create fake sink
                    trace!("creating new fake sink...");
                    let fake_sink = gst::ElementFactory::make(
                        "fakesink",
                        Some(&format!("fakesink-{name}", name = participant.name)),
                    )
                    .unwrap();
                    fake_sink.set_property_from_str("sync", "true");
                    if let Some(peer) = participant.video_src_pad.peer() {
                        trace!(
                            "unlinking compositor {sink} from {source}...",
                            sink = peer.name(),
                            source = name
                        );
                        participant.video_src_pad.unlink(&peer).unwrap();

                        trace!("add fake sink to pipeline...");
                        self.pipeline.add(&fake_sink).unwrap();
                        trace!("create fake sink pad...");
                        trace!("link to fake sink pad...");
                        participant
                            .video_src_pad
                            .link(&fake_sink.static_pad("sink").unwrap())
                            .unwrap();
                    }
                    participant.video_sink_pad = fake_sink.static_pad("sink").unwrap();
                    participant.video_fake_sink = Some(fake_sink);
                    self.visibles = self.visibles - 1;
                    trace!("unlinked {name} successfully", name = participant.name);
                }
            } else {
                return Err(Error::ParticipantNotFound(name.clone()));
            }
        }
        Ok(())
    }
}

pub fn generate_dot_file(pipeline: &gst::Pipeline, filename_without_extension: &str) {
    if let Ok(path) = std::env::var("GST_DEBUG_DUMP_DOT_DIR") {
        info!(
            "writing DOT file `{}/{filename_without_extension}.dot`...",
            path
        );
        gst::debug_bin_to_dot_file(
            pipeline,
            gst::DebugGraphDetails::ALL,
            filename_without_extension,
        );
    } else {
        warn!("can not write DOT file. You need to set GST_DEBUG_DUMP_DOT_DIR in environment to a absolute path");
    }
}
