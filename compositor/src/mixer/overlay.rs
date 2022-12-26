use gst::prelude::*;

pub trait OverlayTrait {
    fn overlay(&self) -> crate::Overlay;
    fn add(&self, pipeline: &gst::Pipeline) {
        pipeline
            .add(self.element())
            .expect("failed to add text overlay to pipeline");
    }
    fn remove(&self, pipeline: &gst::Pipeline) {
        pipeline
            .remove(self.element())
            .expect("failed to remove text overlay to pipeline");
    }
    fn element(&self) -> &gst::Element;
    fn src(&self) -> gst::Pad;
    fn sink(&self) -> gst::Pad;
}
