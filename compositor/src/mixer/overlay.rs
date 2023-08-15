use crate::*;

/// Trait of overlays as the mixer sees it.
pub trait OverlayTrait {
    /// Add overlay element
    fn element(&self) -> &gst::Element;
    /// show or hide overlay element
    fn show(&self, show: bool);
}

/// enum which bundles several types of overlays
#[derive(Debug, Clone)]
pub enum Overlay {
    /// Text overlay
    Text(TextOverlay),
    /// Clock overlay
    Clock(ClockOverlay),
}

impl OverlayTrait for Overlay {
    fn element(&self) -> &gst::Element {
        match self {
            Self::Text(o) => o.element(),
            Self::Clock(o) => o.element(),
        }
    }
    fn show(&self, show: bool) {
        match self {
            Self::Text(o) => o.show(show),
            Self::Clock(o) => o.show(show),
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
