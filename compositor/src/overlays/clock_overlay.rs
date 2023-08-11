use crate::*;
use gst::prelude::*;

/// Overlay displaying a current time.
#[derive(Debug, Clone)]
pub struct ClockOverlay {
    element: gst::Element,
}

impl ClockOverlay {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(format: &str, text_format: TextFormat) -> ClockOverlay {
        trace!("new( {format:?}, {text_format:?} )");

        // create text overlay
        let element = gst::ElementFactory::make_with_name("clockoverlay", None)
            .expect("failed to create clock overlay");

        // set up properties
        element.set_property("time-format", format);
        element.set_property(
            "font-desc",
            format!(
                "{name},{size}",
                name = text_format.font.name,
                size = text_format.font.size
            ),
        );
        element.set_property("xpad", text_format.padding.x);
        element.set_property("ypad", text_format.padding.y);
        element.set_property("color", text_format.color);
        element.set_property_from_str("halignment", text_format.align.horizontal.into());
        element.set_property_from_str("valignment", text_format.align.vertical.into());

        // return Overlay
        Self { element }
    }
}

impl OverlayTrait for ClockOverlay {
    fn element(&self) -> &gst::Element {
        &self.element
    }
    fn show(&self, show: bool) {
        self.element.set_property("silent", !show);
    }
}
