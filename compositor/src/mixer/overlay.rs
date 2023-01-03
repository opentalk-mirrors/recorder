use crate::*;
use gst::prelude::*;

/// Trait of overlays as the mixer sees it.
pub trait OverlayTrait {
    /// Add overlay element
    fn add_to(&self, bin: &gst::Bin);
    fn remove(&self, bin: &gst::Bin);
    fn src(&self) -> gst::Pad;
    fn sink(&self) -> gst::Pad;
}

/// enum which bundles several types of overlays
#[derive(Debug, Clone)]
pub enum Overlay {
    Text(TextOverlay),
    Clock(ClockOverlay),
}

impl OverlayTrait for Overlay {
    fn add_to(&self, bin: &gst::Bin) {
        match self {
            Self::Text(o) => o.add_to(bin),
            Self::Clock(o) => o.add_to(bin),
        }
    }
    fn remove(&self, bin: &gst::Bin) {
        match self {
            Self::Text(o) => o.remove(bin),
            Self::Clock(o) => o.remove(bin),
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

#[derive(Clone, Debug)]
pub struct Overlays {
    /// The mixer GStreamer pipeline.
    bin: gst::Bin,
    /// pad on which the first (lowest z-order) overlay's sink pad has to be attached to
    overlay_src: gst::Pad,
    /// pad on which the last (highest z-order) overlay's src pad has to be attached to
    overlay_sink: gst::Pad,
    /// on top overlays
    overlays: Vec<Overlay>,
}

impl Overlays {
    pub fn new(bin: &gst::Bin, overlay_src: gst::Pad, overlay_sink: gst::Pad) -> Self {
        Self {
            bin: bin.clone(),
            overlay_src,
            overlay_sink,
            overlays: Vec::new(),
        }
    }
    pub fn last(&self) -> Option<&Overlay> {
        self.overlays.last()
    }
    /// push new overlay on top of output video within the pipeline
    /// # Arguments
    /// - `overlay`: new overlay to push
    pub fn push(&mut self, overlay: Overlay) {
        debug!("add overlay: {:?}", overlay);

        // get last overlay source
        let last_src = match self.overlays.last() {
            Some(overlay) => overlay.src(),
            None => self.overlay_src.clone(),
        };
        // get sink the overall output goes to
        let output_sink = &self.overlay_sink;

        // add new element to pipeline
        overlay.add_to(&self.bin);

        // unlink last overlay source pad from output sink
        if let Some(last) = output_sink.peer() {
            last.unlink(output_sink)
                .expect("failed to unlink last overlay element or compositor from output sink");
            trace!("link {} with {}", last_src.name(), overlay.sink().name());
            // link previous overlay source pad to the new overlay's sink
            last_src
                .link(&overlay.sink())
                .expect("failed to link previous src to new overlay's sink");
        }
        // link new overlay's source pad to output sink
        overlay
            .src()
            .link(output_sink)
            .expect("failed to link new overlay source pad to output sink");

        // remember this overlay
        self.overlays.push(overlay);
    }
}
