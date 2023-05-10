use crate::{debug, dynamic, mixer::dynamic::*, testing};
use glib::Cast;
use gst::prelude::*;

/// Sleep every repetition for the given amount of milliseconds.
#[cfg(test)]
const WAIT_MS: u64 = 0;
/// Repeat test the given amount of times.
const REPEATS: usize = 1000;

#[test]
fn scenario1() {
    test("scenario1", &test_scenario1, &build)
}

/// Scenario 1 Test
pub fn test_scenario1(dot: &mut Dot, pipeline: &gst::Pipeline, _: std::time::Duration) {
    // link source
    {
        // redirect source from compositor to a fakesink
        let compositor = pipeline.by_name("compositor").unwrap();
        let valve = pipeline.by_name("source-valve").unwrap();

        link_source(&valve, &compositor).unwrap();
        flush_bus(pipeline);

        dot.make(pipeline, "result");
        testing::wait_short();
    }
    // unlink source
    {
        let compositor = pipeline.by_name("compositor").unwrap();
        let valve = pipeline.by_name("source-valve").unwrap();

        unlink_source(&valve, &compositor).unwrap();
        flush_bus(pipeline);

        dot.make(pipeline, "backwards-result");
        testing::wait_short();
    }
}

#[test]
fn scenario2() {
    test("scenario2", &test_scenario2, &build)
}

/// Scenario 2 Test
pub fn test_scenario2(dot: &mut Dot, pipeline: &gst::Pipeline, _: std::time::Duration) {
    // remove linked source
    {
        let bin: gst::Bin = pipeline.by_name("source").and_dynamic_cast().unwrap();
        let inp = bin.by_name("source-inp").unwrap();
        let inp_src = inp.static_pad("src").unwrap();
        let valve = pipeline.by_name("source-valve").unwrap();
        let compositor = pipeline.by_name("compositor").unwrap();

        remove_source(inp_src, &valve, &compositor).unwrap();
        remove_valve(valve).unwrap();
        remove_bin(bin).unwrap();
        flush_bus(pipeline);

        dot.make(pipeline, "result");
        testing::wait_short();
    }
    // add source amd link
    {
        let compositor = pipeline.by_name("compositor").unwrap();

        let bin: gst::Bin = gst::parse_bin_from_description(
            "
            name=source

            videotestsrc
                name=source-inp
                is-live=true
            ! video/x-raw, width=320, height=240, format={{I420,YV12,YUY2,UYVY,AYUV}}
            ! queue
                name=source-out
        ",
            false,
        )
        .unwrap();

        let out_src = bin
            .by_name("source-out")
            .unwrap()
            .static_pad("src")
            .unwrap();
        let ghost_pad = gst::GhostPad::with_target(Some("source-out"), &out_src)
            .expect("failed to create ghost pad for webrtc video output");
        bin.add_pad(&ghost_pad)
            .expect("failed to add video output ghost pad to webrtc bin");

        pipeline.add(&bin).unwrap();

        let out_src = bin
            .by_name("source-out")
            .unwrap()
            .static_pad("src")
            .unwrap();
        let ghost_pad = out_src
            .peer()
            .and_dynamic_cast::<gst::ProxyPad>()
            .unwrap()
            .internal()
            .and_dynamic_cast::<gst::GhostPad>()
            .unwrap();
        let valve = add_source(&bin, &ghost_pad, Some("source-valve")).unwrap();
        link_source(&valve, &compositor).unwrap();
        flush_bus(pipeline);

        dot.make(pipeline, "backwards-result");
        testing::wait_short();
    }
}

// internal count12ers do not touch!

pub struct Dot {
    count: usize,
    repeat: usize,
    scenario: String,
}
const DOT_LAST_ONLY: bool = true;

impl Dot {
    /// Write DOT diagram of pipeline into a file with the given name
    pub fn make(&mut self, pipeline: &gst::Pipeline, name: &str) {
        let filename = if DOT_LAST_ONLY {
            format!(
                "{scenario}-LAST-{count}-{name}",
                scenario = self.scenario,
                count = self.count
            )
        } else {
            if self.repeat > 1 && self.repeat < REPEATS {
                return;
            }
            format!(
                "{scenario}-{repeat}-{count}-{name}",
                scenario = self.scenario,
                repeat = self.repeat,
                count = self.count
            )
        };
        self.count += 1;

        debug::dot_ext(pipeline, &filename, testing::DOT_PARAMS);
    }
    /// Initialize name of scenario which is used for naming the DOT files
    fn scenario(scenario: &str) -> Dot {
        Dot {
            count: 1,
            repeat: 0,
            scenario: scenario.into(),
        }
    }
    /// Do one repetition step.
    fn repeat(dot: &mut Dot) {
        dot.repeat += 1;
        dot.count = 0;
    }
}

/// Call this function from your own unit test
#[cfg(test)]
fn test(
    scenario: &str,
    step: &dyn Fn(&mut Dot, &gst::Pipeline, std::time::Duration),
    build: &dyn Fn(&mut Dot) -> gst::Pipeline,
) {
    use std::time::Duration;

    testing::init();

    let mut dot = Dot::scenario(scenario);

    info!("Setting up pipeline...");
    let pipeline = build(&mut dot);
    std::thread::sleep(Duration::from_millis(1));

    Dot::repeat(&mut dot);
    dot.make(&pipeline, "original");

    for n in 1..REPEATS + 1 {
        info!("\n--------------------- {n} ---------------------\n");
        step(&mut dot, &pipeline.clone(), Duration::from_millis(WAIT_MS));
        Dot::repeat(&mut dot);
    }

    info!("Finished.");

    pipeline.set_state(gst::State::Null).unwrap();
    flush_bus(&pipeline);
}

// Build test pipeline including two video sources, a compositor and a sink
pub fn build(dot: &mut Dot) -> gst::Pipeline {
    // create pipeline
    let pipeline = gst::parse_launch(&format!(
        r#"

        bin.(
            name=other
            videotestsrc
                name=other-inp
                is-live=true
            ! video/x-raw, width=320, height=240, format={{I420, YV12, YUY2, UYVY, AYUV}}
            ! queue
                name=other-out
        )

        bin.(
            name=source
            videotestsrc
                name=source-inp
                is-live=true
            ! video/x-raw, width=320, height=240, format={{I420, YV12, YUY2, UYVY, AYUV}}
            ! queue
                name=source-out
        )

        compositor
            name=compositor
        ! video/x-raw, width=320, height=240, format={{I420, YV12, YUY2, UYVY, AYUV}}
        ! videoconvert
        ! {sink}
        "#,
        sink = if std::env::var("USE_DISPLAY").is_ok() {
            "xvimagesink"
        } else {
            "fakesink"
        }
    ))
    .unwrap()
    .downcast::<gst::Pipeline>()
    .expect("not a pipeline");

    // debug pipeline
    dot.make(&pipeline, "pipeline_ready");

    // get elements of source bin
    let bin = pipeline
        .by_name("source")
        .and_dynamic_cast::<gst::Bin>()
        .unwrap();
    let out_src = bin
        .by_name("source-out")
        .unwrap()
        .static_pad("src")
        .unwrap();
    let ghost_pad = gst::GhostPad::with_target(Some("source-out"), &out_src)
        .expect("failed to create ghost pad for webrtc video output");
    bin.add_pad(&ghost_pad)
        .expect("failed to add video output ghost pad to webrtc bin");

    // get elements of other bin
    let other_bin = pipeline
        .by_name("other")
        .and_dynamic_cast::<gst::Bin>()
        .unwrap();
    let other_out_src = other_bin
        .by_name("other-out")
        .unwrap()
        .static_pad("src")
        .unwrap();
    let other_ghost_pad = gst::GhostPad::with_target(Some("other-out"), &other_out_src)
        .expect("failed to create ghost pad for webrtc video output");
    other_bin
        .add_pad(&other_ghost_pad)
        .expect("failed to add video output ghost pad to webrtc bin");

    // get compositor
    let compositor = pipeline.by_name("compositor").unwrap();

    // add source bin
    crate::dynamic::add_source(&bin, &ghost_pad, Some("source-valve")).unwrap();

    // add other bin and link valve to compositor
    let other_valve =
        crate::dynamic::add_source(&other_bin, &other_ghost_pad, Some("other-valve")).unwrap();
    dynamic::link_source(&other_valve, &compositor).unwrap();

    // auto layout via pad-added signal
    compositor.connect("pad-added", true, move |args| {
        let pad = args[1].get::<gst::Pad>().unwrap();

        pad.set_property_from_str("width", "160");
        pad.set_property_from_str("height", "120");

        if pad.name() == "sink_0" {
            pad.set_property_from_str("xpos", "160");
            pad.set_property_from_str("ypos", "120");
        }
        None
    });

    // set pipeline to Playing
    pipeline.set_state(gst::State::Playing).unwrap();

    // debug pipeline
    dot.make(&pipeline, "pipeline_playing");

    pipeline
}

pub fn flush_bus(pipeline: &gst::Pipeline) {
    for _ in pipeline.bus().unwrap().iter() {}
}
