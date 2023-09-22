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
    fn generate_ids<ID>(count: u32) -> Vec<(ID, String)>
    where
        ID: Eq + Ord + Hash + Copy + Debug + From<u32>,
    {
        trace!("generate_ids( {count} )");

        // generate stream IDs and names
        (0..count)
            .map(|n| (n.into(), format!("Participant {n:?}")))
            .collect()
    }

    /// generate given number of participant streams
    pub fn generate_streams<ID>(
        talk: &mut Talk<TestSource, ID>,
        count: u32,
        visibles: usize,
    ) -> (Vec<(ID, String)>, Vec<ID>)
    where
        ID: Eq + Ord + Hash + Copy + Debug + Display + From<u32> + Sync + Send,
    {
        trace!("generate_streams( {count}, {visibles} )");

        let streams = generate_ids(count);
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
            };
            talk.add_stream(StreamId::camera(*id), name, params, StreamStatus::default())
                .unwrap();
        }

        talk.layout::<Speaker>().unwrap();

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

    /// Fake sink to catch the compositor output without any further processing.
    #[derive(Debug)]
    pub enum TestSink {
        Fake(FakeSink),
        Display(DisplaySink),
    }

    impl TestSink {
        /// Create and add new fake sink into existing pipeline.
        pub fn new(name: &str) -> Self {
            trace!("new({name})");

            if use_display() {
                info!("using display sink because display is available");
                Self::Display(DisplaySink::new(name))
            } else {
                info!("using fake sink");
                Self::Fake(FakeSink::new(name))
            }
        }
    }

    impl Default for TestSink {
        fn default() -> Self {
            Self::new("Test Sink")
        }
    }

    impl Sink for TestSink {
        fn bin(&self) -> gst::Bin {
            match self {
                Self::Fake(sink) => sink.bin(),
                Self::Display(sink) => sink.bin(),
            }
        }
        /// Get video sink pad.
        fn video(&self) -> gst::GhostPad {
            match self {
                Self::Fake(sink) => sink.video(),
                Self::Display(sink) => sink.video(),
            }
        }

        /// Get audio sink pad.
        fn audio(&self) -> gst::GhostPad {
            match self {
                Self::Fake(sink) => sink.audio(),
                Self::Display(sink) => sink.audio(),
            }
        }
    }
}
