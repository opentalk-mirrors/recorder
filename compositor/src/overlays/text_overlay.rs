use crate::*;
use gst::prelude::*;

/// Text overlay.
#[derive(Debug, Clone)]
pub struct TextOverlay {
    element: gst::Element,
}

impl TextOverlay {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(text: &str, text_format: TextFormat) -> TextOverlay {
        trace!("new( '{text}', {text_format:?} )");

        // create text overlay
        let element = gst::ElementFactory::make_with_name("textoverlay", None)
            .expect("failed to create text overlay");

        // set up properties
        element.set_property("text", text);
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
    pub fn set(&self, text: &str) {
        trace!("set( '{text}' )");

        self.element.set_property("text", text);
    }
}

impl OverlayTrait for TextOverlay {
    fn element(&self) -> &gst::Element {
        &self.element
    }
    fn show(&self, show: bool) {
        self.element.set_property("silent", !show);
    }
}
