// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

mod blinder;
mod dash;
mod generate_example_pipeline_picture;
mod matroska;
mod mixer;
mod mp4;
mod multi;
mod overlays;
mod speaker_mode;
mod stream_status;
mod webrtc;

pub mod testing {

    use crate::*;
    use core::{
        fmt::{Debug, Display},
        hash::Hash,
        time::Duration,
    };
    use std::sync::Once;

    /// output resolution to use when creating Mixer for testing
    pub const RESOLUTION: Size = Size::HD;
    /// GStreamer debug details to use when generating DOT files of pipeline within testing
    pub const DOT_PARAMS: &debug::Params = &debug::Params::all();

    // The maximum amount of streams which will be used in tests.
    pub const MAX_STREAMS: usize = 100;

    static INIT: Once = Once::new();

    /// initialize for testing
    pub fn init() {
        trace!("init()");
        INIT.call_once(init_function);
    }

    fn init_function() {
        if let Ok(path) = std::env::var("GST_DEBUG_DUMP_DOT_DIR") {
            debug!("Removing any *.dot files in {path}");
            for path in glob::glob(&(path.to_string() + "/*.dot")).unwrap() {
                match path {
                    Ok(path) => std::fs::remove_file(path).unwrap(),
                    Err(err) => error!("path not found: {err:?}"),
                }
            }
        }
        // initialize gstreamer
        gst::init().unwrap();
        // init logger
        env_logger::try_init().ok();

        if use_display() {
            info!("Showing output in window and playing sound (USE_DISPLAY or USER_TEST)");
        }
        if use_display() {
            info!("Slowing down tests for user observation (BE_SLOW or USER_TEST)");
        }

        debug!("Current directory {:?}", std::env::current_dir().unwrap());
        info!("Output directory: {}", output_dir());
        info!("Image directory: {}", image_dir());

        std::thread::spawn({
            let main_loop = glib::MainLoop::new(None, false);
            move || {
                main_loop.run();
            }
        });
    }

    fn be_slow() -> bool {
        std::env::var("USER_TEST").is_ok() || std::env::var("BE_SLOW").is_ok()
    }

    /// return true if system provides a display
    fn use_display() -> bool {
        (std::env::var("USER_TEST").is_ok() || std::env::var("USE_DISPLAY").is_ok())
            && std::env::var("DISPLAY").is_ok()
    }

    /// get output directory depending if we are within the compositor module or above
    fn base_dir() -> &'static str {
        if std::env::current_dir().unwrap().ends_with("compositor") {
            "."
        } else {
            "./compositor"
        }
    }
    /// get output directory depending if we are within the compositor module or above
    pub fn output_dir() -> String {
        format!("{}/test_output", base_dir())
    }

    /// get output directory depending if we are within the compositor module or above
    pub fn output_file(filename: &str) -> String {
        format!("{}/{filename}", output_dir())
    }

    /// get output directory depending if we are within the compositor module or above
    pub fn image_dir() -> String {
        format!("{}/images", base_dir())
    }

    /// get output directory depending if we are within the compositor module or above
    pub fn image_file(filename: &str) -> String {
        format!("{}/{filename}", image_dir())
    }

    /// generate IDs for given amount of participants
    fn generate_ids<ID>(first: u32, count: u32) -> Vec<(ID, String)>
    where
        ID: Eq + Ord + Hash + Copy + Debug + From<u32>,
    {
        trace!("generate_ids( {count} )");

        // generate stream IDs and names
        (first..(first + count))
            .map(|n| (n.into(), format!("Participant {n:?}")))
            .collect()
    }

    /// generate given number of participant streams
    pub fn generate_streams<ID>(
        talk: &mut Talk<TestSource, ID>,
        first: u32,
        count: u32,
        visibles: usize,
        has_video: bool,
    ) -> (Vec<(ID, String)>, Vec<ID>)
    where
        ID: Eq + Ord + Hash + Copy + Debug + Display + From<u32> + Sync + Send,
    {
        trace!("generate_streams( {count}, {visibles} )");

        let streams = generate_ids(first, count);
        let ids: Vec<ID> = streams.iter().map(|p| p.0).collect();

        let resolutions = [Size::SD, Size::HD, Size::FHD, Size::QHD, Size::UHD];
        let images = [
            "participant_SD.png",
            "participant_HD.png",
            "participant_FHD.png",
            "participant_QHD.png",
            "participant_UHD.png",
        ];

        for (i, (id, name)) in streams.iter().enumerate() {
            let params = TestSourceParameters {
                resolution: resolutions[i % images.len()],
                pattern: Pattern::Location(testing::image_file(images[i % images.len()])),
                name: Some(name.clone()),
                has_video,
            };
            talk.add_stream(StreamId::camera(*id), name, params, StreamStatus::default())
                .unwrap();
        }

        (streams, ids)
    }

    /// wait the given amount of seconds
    pub fn wait_secs(sec: u64) {
        info!("-- waiting {sec} second(s) --");
        std::thread::sleep(Duration::from_secs(sec));
    }

    /// wait the given amount of milliseconds
    pub fn wait_millis(milliseconds: u64) {
        info!("-- waiting {milliseconds} millisecond(s) --");
        std::thread::sleep(Duration::from_millis(milliseconds));
    }

    /// wait 3s if display is present, else wait 200ms
    pub fn wait() {
        let milliseconds = if be_slow() { 3000 } else { 200 };
        info!("-- waiting {milliseconds} millisecond(s) --");
        std::thread::sleep(Duration::from_millis(milliseconds));
    }

    /// like `wait()` but waits 200ms or zero time
    pub fn wait_short() {
        if be_slow() {
            let milliseconds = 200;
            info!("-- waiting {milliseconds} millisecond(s) --");
            std::thread::sleep(Duration::from_millis(milliseconds));
        }
    }
}
