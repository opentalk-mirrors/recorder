//! Functions to manipulate a playing pipeline

// what we need from external libraries

use crate::debug;
use anyhow::{anyhow, Result};
use gst::{prelude::*, PadProbeInfo, PadProbeReturn, PadProbeType};
use std::sync::mpsc;

/// Safely unlink source from compositor or mixer.
/// # Arguments
/// - `valve`: Dropping valve element providing the output of a source
/// - `target`: Element to disconnect from
pub fn unlink_source(valve: &gst::Element, target: &gst::Element) -> Result<()> {
    trace!(
        "unlink_source({valve}, {target})",
        valve = debug::name(valve),
        target = debug::name(target),
    );
    debug::dot(target, "unlink_source");

    let valve_src = valve
        .static_pad("src")
        .ok_or_else(|| anyhow!("valve src sink not found"))?;

    let Some(sink) = valve_src.peer() else {
        warn!("Unnecessary unlink!");
        return Ok(());
    };

    // close valve
    valve.set_property("drop", true);

    // unlink ghosting from target sink
    valve_src.unlink(&sink)?;

    // remove mixer pad
    target.release_request_pad(&sink);

    Ok(())
}

/// Safely link unlinked source to target to compositor or mixer
/// # Arguments
/// - `valve`: Dropping valve element providing the output of a source
/// - `target`: Element to connect to
pub fn link_source(valve: &gst::Element, target: &gst::Element) -> Result<gst::Pad> {
    trace!(
        "link_source( {valve}, {target})",
        valve = debug::name(valve),
        target = debug::name(target),
    );
    debug::dot(target, "link_source");

    let valve_src = valve
        .static_pad("src")
        .ok_or_else(|| anyhow!("valve src sink not found"))?;

    if let Some(sink) = valve_src.peer() {
        warn!("Unnecessary link!");
        // return sink pad at target
        return Ok(sink);
    }

    // get a new sink pad from the target
    let sink = target
        .request_pad_simple("sink_%u")
        .ok_or_else(|| anyhow!("Could not request pad from '{name}'", name = target.name()))?;
    // link to target
    valve_src.link(&sink)?;
    // close valve
    valve.set_property("drop", false);
    // return sink pad at target
    Ok(sink)
}

/// Safely remove a linked source (including it's valve) from pipeline.
/// # Arguments
/// - `inp_src`: Pad to flush from
/// - `valve`: Dropping valve element providing the output of the source
/// - `target`: Element to connect to
pub fn remove_source(inp_src: gst::Pad, valve: &gst::Element, target: &gst::Element) -> Result<()> {
    trace!(
        "remove_source({inp_src}, {valve}, {target})",
        inp_src = debug::name(&inp_src),
        valve = debug::name(valve),
        target = debug::name(target),
    );

    debug::dot(target, "remove_source");

    // get sink behind the source's input element
    let after_inp_sink = inp_src
        .peer()
        .ok_or_else(|| anyhow!("inp element not connected"))?;
    let valve_sink = valve
        .static_pad("sink")
        .ok_or_else(|| anyhow!("sink of valve not found"))?;

    let valve_src = valve
        .static_pad("src")
        .ok_or_else(|| anyhow!("valve src pad not found"))?;

    let Some(ghost_pad) = valve_sink.peer() else {
        warn!("no ghost_pad found in remove_source!");
        return Ok(());
    };

    let Some(target_sink) = valve_src.peer() else {
        warn!("no target_sink found in remove_source!");
        return Ok(());
    };

    // prepare channel to sync with event probe
    let (event_sender, event_receiver) = mpsc::sync_channel::<bool>(1);

    let eos_handler = move |ghost_pad: &gst::Pad, info: &mut PadProbeInfo| match &info.data {
        // Act on EOS events
        Some(gst::PadProbeData::Event(event)) if { event.type_() == gst::EventType::Eos } => {
            // remove event probe from ghost pad
            if let Some(probe_id) = info.id.take() {
                // remove event probe from ghost pad
                ghost_pad.remove_probe(probe_id);
            } else {
                error!(
                    "Failed to find probe_id {:?} on pad {:?}. Unable to remove EOS probe. ",
                    info, ghost_pad
                );
            }

            // synchronize with caller
            event_sender
                .send(true)
                .expect("could not synchronize with caller");
            PadProbeReturn::Drop
        }
        _ => PadProbeReturn::Ok,
    };

    // close valve
    valve.set_property("drop", true);

    // add event probe at ghost pad
    ghost_pad.add_probe(PadProbeType::EVENT_DOWNSTREAM, eos_handler);

    // send EOS and wait until it arrives at the ghost pad
    after_inp_sink.send_event(gst::event::Eos::new());
    event_receiver.recv()?;

    // unlink valve from target
    valve_src.unlink(&target_sink)?;
    if target.pads().contains(&target_sink) {
        target.release_request_pad(&target_sink);
    }
    Ok(())
}

pub fn remove_valve(valve: gst::Element) -> Result<()> {
    // remove valve from pipeline
    if gst::StateChangeSuccess::Async == valve.set_state(gst::State::Null)? {
        warn!("remove_source: async state change")
    }
    valve
        .parent()
        .and_dynamic_cast::<gst::Bin>()
        .expect("expected parent of valve to be a bin")
        .remove(&valve)?;

    Ok(())
}

/// Safely remove unlinked bin from pipeline.
pub fn remove_bin(bin: gst::Bin) -> Result<()> {
    if gst::StateChangeSuccess::Async == bin.set_state(gst::State::Null)? {
        warn!("remove_source: async state change")
    }
    let pipeline: gst::Pipeline = bin
        .parent()
        .and_dynamic_cast()
        .expect("expect parent of bin to be a pipeline");
    pipeline.remove(&bin)?;
    Ok(())
}

/// Safely add unlinked source to pipeline.
pub fn add_source(
    bin: &gst::Bin,
    ghost_pad: &gst::GhostPad,
    valve_name: Option<&str>,
) -> Result<gst::Element> {
    trace!(
        "add_source({bin}, {ghost_pad})",
        bin = debug::name(bin),
        ghost_pad = debug::name(ghost_pad),
    );

    // prepare closed valve
    let valve = gst::ElementFactory::make_with_name("valve", valve_name)?;
    valve.set_property("drop", true);

    // add valve outside the bin
    bin.parent()
        .and_dynamic_cast::<gst::Bin>()
        .expect("expecting parent of valve to be a bin")
        .add(&valve)?;
    valve.sync_state_with_parent()?;

    // link source to valve's sink pad
    let valve_sink = valve
        .static_pad("sink")
        .ok_or_else(|| anyhow!("valve has no sink pad"))?;
    ghost_pad.link(&valve_sink)?;

    // (re-)start bin
    bin.set_state(gst::State::Playing)?;

    Ok(valve)
}

/// Safely insert a new element between two others.
pub fn insert_element(bin: &gst::Bin, valve: &gst::Element, element: gst::Element) -> Result<()> {
    trace!(
        "insert_element({at_valve}, {element})",
        at_valve = debug::name(valve),
        element = debug::name(&element)
    );
    debug::dot(bin, "insert_element");

    // get source pad behind given sink pad
    let valve_src = valve
        .static_pad("src")
        .ok_or_else(|| anyhow!("src pad of valve not found"))?;
    let next_sink = valve_src
        .peer()
        .ok_or_else(|| anyhow!("peer of src pad of valve not found"))?;
    let next_element = &next_sink
        .parent_element()
        .ok_or_else(|| anyhow!("next element not found"))?;

    // close valve
    valve.set_property("drop", true);

    // disconnect the pads which will surround new element
    valve_src.unlink(&next_sink)?;

    // add element to bin
    bin.add(&element)?;

    // start element
    element.sync_state_with_parent()?;

    // link all together
    valve.link(&element)?;

    element.link(next_element)?;

    // open valve
    valve.set_property("drop", false);

    Ok(())
}

/// Place probe at `src_pad` which calls `f` when `EOS` arrives at it.
///
/// # Arguments
///
/// - `src_pad`: Pad to place the probe at.
/// - `once`: If `true` probe will be removed after first `EOS` event.
/// - `f`: Call back function.
///
pub fn on_eos<F>(src_pad: &gst::Pad, once: bool, f: F)
where
    F: Fn() + Sync + Send + 'static,
{
    trace!("on_eos({src_pad}, {once})", src_pad = debug::name(src_pad));

    let eos_handler = move |src_pad: &gst::Pad, info: &mut PadProbeInfo| match &info.data {
        // Act on EOS events
        Some(gst::PadProbeData::Event(event)) if { event.type_() == gst::EventType::Eos } => {
            // remove event probe from ghost pad
            if once {
                if let Some(probe_id) = info.id.take() {
                    src_pad.remove_probe(probe_id);
                } else {
                    error!(
                        "Failed to find probe_id {:?} on pad {:?}. Unable to remove EOS probe. ",
                        info, src_pad
                    );
                }
            }

            f();
            PadProbeReturn::Ok
        }
        _ => PadProbeReturn::Ok,
    };

    // add event probe at src pad
    src_pad.add_probe(PadProbeType::EVENT_DOWNSTREAM, eos_handler);
}
