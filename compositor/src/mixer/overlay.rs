use crate::*;
use anyhow::Result;
use glib::CastNone;
use gst::traits::GstObjectExt;

/// Trait of overlays as the mixer sees it.
pub trait OverlayTrait {
    /// Add overlay element
    fn element(&self) -> gst::Element;
    /// Return source pad.
    fn src(&self) -> gst::Pad;
    /// Return sink pad.
    fn sink(&self) -> gst::Pad;
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
    fn element(&self) -> gst::Element {
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

/// List of overlays currently attached to the pipeline.
#[derive(Clone, Debug)]
pub struct Overlays {
    /// pad to which the last (highest z-order in composite) overlay shall be linked to
    valve: gst::Element,
    /// on top overlays
    overlays: Vec<Overlay>,
}

impl Overlays {
    /// Create new overlay container wich overlays will be added before the given `sink`.
    pub fn new(valve: gst::Element) -> Self {
        Self {
            valve,
            overlays: Vec::new(),
        }
    }
    pub fn last(&self) -> Option<&Overlay> {
        self.overlays.last()
    }
    /// insert the given overlay on top of output video into the pipeline
    pub fn push(&mut self, overlay: Overlay) -> Result<()> {
        trace!("push( {:?} )", overlay);

        // get the enveloping bin
        let bin: gst::Bin = self
            .valve
            .parent()
            .and_dynamic_cast()
            .expect("expecting parent of valve to be a bin");

        // insert element into running pipeline
        dynamic::insert_element(&bin, &self.valve, overlay.element())?;

        // remember this overlay
        self.overlays.push(overlay);

        Ok(())
    }
}
