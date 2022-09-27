use std::sync::{Arc, Mutex};

use super::display_sink::DisplaySink;

use crate::{layout::*, mixer::*};
use gst::{
    prelude::{ElementExtManual, GObjectExtManualGst, ObjectExt},
    traits::{ElementExt, GstBinExt, GstObjectExt, PadExt},
};
use gstreamer as gst;

#[derive(Debug)]
pub enum Error {
    TooManyParticipants,
}

pub struct Mixer<L>
where
    L: Layout,
{
    elements: Vec<gst::Element>,
    pub compositor: gst::Element,
    resolution: Size,
    max_participants: usize,
    clock: Option<gst::Element>,
    title: Option<gst::Element>,
    speaking: Option<gst::Element>,
    output_pad: gst::Pad,
    pipeline: gst::Pipeline,
    layout: L,
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

        // create compositor
        let compositor =
            gst::ElementFactory::make("compositor", Some(&format!("compositor"))).unwrap();
        compositor.set_property_from_str("background", "black");

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
        pipeline.add(&compositor).unwrap();
        pipeline.add(&clock_overlay).unwrap();
        pipeline.add(&title_overlay).unwrap();
        pipeline.add(&speaking_overlay).unwrap();
        pipeline.add(&output_queue).unwrap();

        // link elements
        background_src.link(&caps).unwrap();
        caps.link(&compositor).unwrap();
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
            clock: Some(clock_overlay.clone()),
            title: Some(title_overlay.clone()),
            speaking: Some(speaking_overlay.clone()),
            output_pad: output_queue.static_pad("src").unwrap(),
            layout,
            pipeline: pipeline.clone(),
        }
    }
    pub fn set_viewable(&self, names: &[&str]) {
        // unlink all compositor pads
    }
    pub fn link_display_sink(&self, sink: &DisplaySink) {
        self.output_pad.link(sink.sink_pad()).unwrap();
    }
    pub fn layout(&self) {
        let count = (self.compositor.num_sink_pads() - 1) as usize;
        self.layout_overlay(
            "title",
            self.layout.title_position(count),
            self.layout.title_alignment(),
        );
        self.layout_overlay(
            "clock",
            self.layout.clock_position(count),
            self.layout.clock_alignment(),
        );
        self.layout_overlay(
            "speaking",
            self.layout.speaking_position(count),
            self.layout.speaking_alignment(count),
        );
        for (n, pad) in self.compositor.sink_pads()[1..].iter().enumerate() {
            if n > 0 {
                let n = n - 1;
                let count = count - 1;
                let pos = self.layout.position(n, count);
                let size = self.layout.size(n, count);
                pad.set_property("xpos", pos.x as i32);
                pad.set_property("ypos", pos.y as i32);
                pad.set_property("width", size.width as i32);
                pad.set_property("height", size.height as i32);
            }
        }
    }
    fn layout_overlay(&self, name: &str, position: Position, alignment: Alignment) {
        if let Some(title) = self.pipeline.by_name(name) {
            title.set_property_from_str("halignment", alignment.horizontal);
            title.set_property_from_str("valignment", alignment.vertical);
            title.set_property_from_str("line-alignment", alignment.horizontal);
            title.set_property_from_str("deltax", &position.x.to_string());
            title.set_property_from_str("deltay", &position.y.to_string());
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
}

pub fn generate_dot_file(pipeline: &gst::Pipeline, filename_without_extension: &str) {
    if let Ok(path) = std::env::var("GST_DEBUG_DUMP_DOT_DIR") {
        info!("writing DOT file `{}/pipeline.dot`...", path);
        gst::debug_bin_to_dot_file(
            pipeline,
            gst::DebugGraphDetails::ALL,
            filename_without_extension,
        );
    } else {
        warn!("can not write DOT file. You need to set GST_DEBUG_DUMP_DOT_DIR in environment to a absolute path");
    }
}
