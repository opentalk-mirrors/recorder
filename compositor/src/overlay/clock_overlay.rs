use crate::*;
use gst::prelude::*;

#[derive(Debug, Clone)]
pub struct ClockOverlay {
    element: gst::Element,
}

impl ClockOverlay {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(format: &str, text_format: TextFormat) -> Overlay {
        // create text overlay
        let element = gst::ElementFactory::make_with_name("clockoverlay", None)
            .expect("failed to create clock overlay");

        // set up properties
        element.set_property("time-format", format);
        element.set_property(
            "font-desc",
            &format!(
                "{name},{size}",
                name = text_format.font.name,
                size = text_format.font.size
            ),
        );
        element.set_property("xpad", text_format.padding.x);
        element.set_property("ypad", text_format.padding.y);
        element.set_property::<u32>("color", text_format.color.into());
        element.set_property_from_str("halignment", text_format.align.horizontal.into());
        element.set_property_from_str("valignment", text_format.align.vertical.into());

        // return Overlay
        Overlay::Clock(Self { element })
    }
}

impl OverlayTrait for ClockOverlay {
    fn add(&self, pipeline: &gst::Pipeline) {
        pipeline
            .add(&self.element)
            .expect("failed to add text overlay to pipeline");
    }
    fn remove(&self, pipeline: &gst::Pipeline) {
        pipeline
            .remove(&self.element)
            .expect("failed to remove text overlay to pipeline");
    }
    fn src(&self) -> gst::Pad {
        self.element
            .static_pad("src")
            .expect("failed to get src pad of clock overlay")
    }
    fn sink(&self) -> gst::Pad {
        self.element
            .static_pad("video_sink")
            .expect("failed to get sink pad of clock overlay")
    }
}
