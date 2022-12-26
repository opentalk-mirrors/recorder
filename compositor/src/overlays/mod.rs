mod text_format;

mod clock_overlay;
mod text_overlay;

pub use text_format::*;

pub use clock_overlay::ClockOverlay;
pub use text_overlay::TextOverlay;

use crate::OverlayTrait;

#[derive(Debug, Clone)]
pub enum Overlay {
    Text(TextOverlay),
    Clock(ClockOverlay),
}

impl OverlayTrait for Overlay {
    fn overlay(&self) -> Overlay {
        self.clone()
    }
    fn element(&self) -> &gst::Element {
        match self {
            Self::Text(o) => o.element(),
            Self::Clock(o) => o.element(),
        }
    }
    fn src(&self) -> gst::Pad {
        match self {
            Self::Text(o) => o.src(),
            Self::Clock(o) => o.src(),
        }
    }
    fn sink(&self) -> gst::Pad {
        match self {
            Self::Text(o) => o.sink(),
            Self::Clock(o) => o.sink(),
        }
    }
}

impl From<TextOverlay> for Overlay {
    fn from(overlay: TextOverlay) -> Overlay {
        Overlay::Text(overlay)
    }
}

impl From<ClockOverlay> for Overlay {
    fn from(overlay: ClockOverlay) -> Overlay {
        Overlay::Clock(overlay)
    }
}
