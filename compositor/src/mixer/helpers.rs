use gst::prelude::*;
use gstreamer as gst;

/// link a pad of an element inside a bin to to a new ghost pad of that bin
/// # Arguments
/// - `bin`: the bin where to find the element
/// - `element_name`: name of the element
/// - `pad_name`:
pub fn link_bin_ghost_pad(bin: &gst::Bin, element_name: &str, pad_name: &str) -> gst::GhostPad {
    let element = bin.by_name(element_name).unwrap();
    let pad = element.static_pad(pad_name).unwrap();
    let ghost_pad = gst::GhostPad::with_target(None, &pad).unwrap();
    // add new ghost pad to bin
    bin.add_pad(&ghost_pad).unwrap();
    ghost_pad
}

/// add and link a new pad of an element inside a bin to to a new ghost pad of that bin
/// # Arguments
/// - `bin`: the bin where to find the element
/// - `element_name`: name of the element
/// - `pad_name`: name of the pad
#[allow(dead_code)]
pub fn link_bin_add_ghost_pad(bin: &gst::Bin, element_name: &str, pad_name: &str) -> gst::GhostPad {
    let element = bin.by_name(element_name).unwrap();
    let pad = element.request_pad_simple(pad_name).unwrap();
    let ghost_pad = gst::GhostPad::with_target(None, &pad).unwrap();
    // add new ghost pad to bin
    bin.add_pad(&ghost_pad).unwrap();
    ghost_pad
}
