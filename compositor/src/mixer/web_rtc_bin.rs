use super::helpers::*;
use super::layout::*;
use gst::prelude::*;
use gstreamer as gst;

#[allow(dead_code)]
pub fn create_web_rtc_bin(
    pipeline: &gst::Pipeline,
    name: &str,
    resolution: &Size,
) -> (gst::GhostPad, gst::GhostPad) {
    // prepare a bin with the dash recorder
    let bin = format!(
        r#"
    webrtcbin
        name=webrtc-{name}
    ! capssetter 
        name={name}
        caps=video/x-raw,width={width},height={height}
    "#,
        width = resolution.width,
        height = resolution.height,
    );

    // parse bin and add it to the pipeline
    info!("parsing test source bin `{name}`:\n{bin}");
    let bin = gst::parse_bin_from_description(&bin, false).unwrap();
    pipeline.add(&bin).unwrap();

    // link our internal sink to a ghost pad at the bin's outside
    let pad = link_bin_ghost_pad(&bin, name, "src");
    (pad, gst::GhostPad::new(None, gst::PadDirection::Sink))
}
