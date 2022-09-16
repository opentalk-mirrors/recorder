use super::helpers::*;
use gst::prelude::*;
use gstreamer as gst;

#[allow(dead_code)]
pub fn create_dash_sink(pipeline: &gst::Pipeline) -> (gst::GhostPad, gst::GhostPad) {
    // prepare a bin with the dash recorder
    let bin = &format!(
        r#"
    x265enc
    name=output-video-sink
    ! dashsink.
    
    avenc_aac
        name=output-audio-sink
        ! dashsink.
    
    dashsink 
        name=dashsink 
        dynamic=true
        mpd-baseurl=file:/{dir}/
        muxer=ts
        "#,
        dir = std::env::current_dir().unwrap().display()
    );

    // parse bin and add it to the pipeline
    info!("parsing dash sink bin:\n{bin}");
    let bin = gst::parse_bin_from_description(bin, false).unwrap();
    pipeline.add(&bin).unwrap();

    // link our internal sink to a ghost pad at the bin's outside
    (
        link_bin_ghost_pad(&bin, "output-video-sink", "sink"),
        link_bin_ghost_pad(&bin, "output-audio-sink", "sink"),
    )
}

#[allow(dead_code)]
pub fn create_display_sink(pipeline: &gst::Pipeline) -> (gst::GhostPad, gst::GhostPad) {
    // prepare a bin with the dash recorder
    let bin = &format!(
        r#"
    autovideosink
        name=output-video-sink

    autoaudiosink
        name=output-audio-sink
        "#,
    );

    // parse bin and add it to the pipeline
    info!("parsing test sink bin:\n{bin}");
    let bin = gst::parse_bin_from_description(bin, false).unwrap();
    pipeline.add(&bin).unwrap();

    // link our internal sink to a ghost pad at the bin's outside
    (
        link_bin_ghost_pad(&bin, "output-video-sink", "sink"),
        link_bin_ghost_pad(&bin, "output-audio-sink", "sink"),
    )
}
