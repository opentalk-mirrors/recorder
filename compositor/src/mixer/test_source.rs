use crate::mixer::mixer::*;

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
) -> (gst::Bin, gst::GhostPad, gst::GhostPad) {
    // get fresh pattern
    let pattern = unsafe {
        let p = PATTERNS[PATTERN_COUNT % PATTERNS.len()];
        PATTERN_COUNT += 1;
        p
    };
    create_test_source_with_pattern(pipeline, name, resolution, pattern)
}

#[allow(dead_code)]
pub fn create_test_source_blank(
    pipeline: &gst::Pipeline,
    name: &str,
    resolution: &Size,
) -> (gst::Bin, gst::GhostPad, gst::GhostPad) {
    create_test_source_with_pattern(pipeline, name, resolution, "black")
}

#[allow(dead_code)]
pub fn create_test_source_with_pattern(
    pipeline: &gst::Pipeline,
    name: &str,
    resolution: &Size,
    pattern: &str,
) -> (gst::Bin, gst::GhostPad, gst::GhostPad) {
    // prepare a bin with the dash recorder
    let bin = format!(
        r#"name={name}-testsrc-bin
    videotestsrc
        pattern={pattern}
        is_live=true
    ! capssetter 
        caps=video/x-raw,format=RGB,width={width},height={height}
        name={name}-video
    ! fakesink
        name=video-fakesink
    
    audiotestsrc
        is_live=true
        volume=0.01
        name={name}-audio
    ! fakesink
        name=audio-fakesink
    "#,
        width = resolution.width,
        height = resolution.height,
    );

    // parse bin and add it to the pipeline
    info!("parsing test source bin `{name}`:\n{bin}");
    let bin = gst::parse_bin_from_description(&bin, false).unwrap();
    pipeline.add(&bin).unwrap();

    let video_source = bin.by_name(&format!("{name}-video")).unwrap();
    let audio_source = bin.by_name(&format!("{name}-audio")).unwrap();

    let video_fakesink = bin.by_name(&format!("video-fakesink")).unwrap();
    let audio_fakesink = bin.by_name(&format!("audio-fakesink")).unwrap();

    let audio_ghost_pad = gst::GhostPad::new(Some("audio_src"), gst::PadDirection::Src);
    let video_ghost_pad = gst::GhostPad::new(Some("video_src"), gst::PadDirection::Src);

    bin.add_pad(&audio_ghost_pad).unwrap();
    bin.add_pad(&video_ghost_pad).unwrap();

    audio_ghost_pad.connect(
        "linked",
        true,
        on_linked(
            audio_source.clone(),
            audio_fakesink.clone(),
            audio_ghost_pad.clone(),
        ),
    );

    audio_ghost_pad.connect(
        "unlinked",
        true,
        on_linked(
            audio_source.clone(),
            audio_fakesink.clone(),
            audio_ghost_pad.clone(),
        ),
    );

    video_ghost_pad.connect(
        "linked",
        true,
        on_linked(
            video_source.clone(),
            video_fakesink.clone(),
            video_ghost_pad.clone(),
        ),
    );

    video_ghost_pad.connect(
        "unlinked",
        true,
        on_unlinked(
            video_source.clone(),
            video_fakesink.clone(),
            video_ghost_pad.clone(),
        ),
    );

    // link our internal sink to a ghost pad at the bin's outside
    (bin.clone(), video_ghost_pad, audio_ghost_pad)
}
