use super::helpers::*;
use super::layout::*;
use gst::prelude::*;
use gstreamer as gst;

const PATTERNS: [&str; 11] = [
    "smpte",
    "snow",
    "ball",
    "pinwheel",
    "smpte75",
    "checkers-2",
    "smpte-rp-219",
    "colors",
    "checkers-8",
    "smpte100",
    "checkers-4",
];
static mut PATTERN_COUNT: usize = 0;

#[allow(dead_code)]
pub fn create_test_source(
    pipeline: &gst::Pipeline,
    name: &str,
    resolution: &Size,
) -> (gst::GhostPad, gst::GhostPad) {
    // get fresh pattern
    let pattern = unsafe {
        let p = PATTERNS[PATTERN_COUNT % PATTERNS.len()];
        PATTERN_COUNT += 1;
        p
    };
    // prepare a bin with the dash recorder
    let bin = format!(
        r#"
    videotestsrc
        pattern={pattern}
        is_live=true
    ! capssetter 
        name={name}-video
        caps=video/x-raw,width={width},height={height}

    audiotestsrc
        name={name}-audio
        is_live=true
        volume=0.01
    "#,
        width = resolution.width,
        height = resolution.height,
    );

    // parse bin and add it to the pipeline
    info!("parsing test source bin `{name}`:\n{bin}");
    let bin = gst::parse_bin_from_description(&bin, false).unwrap();
    pipeline.add(&bin).unwrap();

    // link our internal sink to a ghost pad at the bin's outside
    (
        link_bin_ghost_pad(&bin, &format!("{name}-video"), "src"),
        link_bin_ghost_pad(&bin, &format!("{name}-audio"), "src"),
    )
}
