use super::*;
use gst::prelude::*;

#[derive(Debug, Clone)]
pub struct TextOverlay {
    element: gst::Element,
}

impl TextOverlay {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(text: &str, text_format: TextFormat) -> TextOverlay {
        // create text overlay
        let element = gst::ElementFactory::make_with_name("textoverlay", None)
            .expect("failed to create text overlay");

        // set up properties
        element.set_property("text", text);
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
        Self { element }
    }
    pub fn set(&self, text: &str) {
        self.element.set_property("text", text);
    }
}

impl OverlayTrait for TextOverlay {
    fn overlay(&self) -> Overlay {
        Overlay::Text(self.clone())
    }
    fn element(&self) -> &gst::Element {
        &self.element
    }
    fn src(&self) -> gst::Pad {
        self.element
            .static_pad("src")
            .expect("failed to get src pad of text overlay")
    }
    fn sink(&self) -> gst::Pad {
        self.element
            .static_pad("video_sink")
            .expect("failed to get sink pad of text overlay")
    }
}
