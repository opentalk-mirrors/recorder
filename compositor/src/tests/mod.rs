mod dash;
mod dynamic;
mod generate_example_pipeline_picture;
mod matroska;
mod mixer;
mod mp4;
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

    /// output resolution to use when creating Mixer for testing
    pub const RESOLUTION: Size = Size::SD;
    /// GStreamer debug details to use when generating DOT files of pipeline within testing
    pub const DOT_PARAMS: &debug::Params = &debug::Params::all();

    // count calls
    use std::sync::atomic::{AtomicBool, Ordering};
    static INITIALIZING: AtomicBool = AtomicBool::new(false);

    /// initialize for testing
    pub fn init() {
        trace!("init()");

        while INITIALIZING
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {}

        INITIALIZING.store(true, Ordering::SeqCst);
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

        INITIALIZING.store(false, Ordering::SeqCst);
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

    /// create a text overlay which displays the given text which shall be the test name
    pub fn add_overlay_name<SRC, ID>(talk: &mut Talk<SRC, ID>, name: &str)
    where
        SRC: Source,
        SRC::Parameters: Debug,
        ID: Eq + Ord + Hash + Copy + Debug + Display + Sync + Send,
    {
        trace!("add_overlay_name( '{name}' )");

        talk.insert_overlay_text(
            name,
            TextFormat {
                font: Font {
                    size: 9,
                    ..Default::default()
                },
                align: Align {
                    horizontal: HAlign::Left,
                    vertical: VAlign::Bottom,
                },
                ..Default::default()
            },
        )
        .unwrap();
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
        mixer: &mut Talk<TestSource, ID>,
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
            mixer
                .add_stream(
                    StreamId::camera(*id),
                    &name,
                    params,
                    StreamStatus::default(),
                )
                .unwrap();
        }

        mixer.layout::<Speaker>().unwrap();

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
    pub enum TestSink {
        Fake(FakeSink),
        Display(DisplaySink),
    }

    pub struct TestSinkBuilder();

    impl TestSinkBuilder {
        pub fn new() -> Self {
            Self()
        }
    }

    impl SinkBuilder for TestSinkBuilder {
        fn build(&self, pipeline: &gst::Pipeline) -> Box<dyn Sink> {
            Box::new(TestSink::new(pipeline))
        }
    }

    impl TestSink {
        /// Create and add new fake sink into existing pipeline.
        pub fn new(pipeline: &gst::Pipeline) -> Self {
            trace!("new()");

            if use_display() {
                info!("using display sink because display is available");
                Self::Display(DisplaySink::new(pipeline))
            } else {
                info!("using fake sink");
                Self::Fake(FakeSink::new(pipeline))
            }
        }
    }

    impl Sink for TestSink {
        /// Get video sink pad.
        fn video_sink_pad(&self) -> gst::Pad {
            match self {
                Self::Fake(sink) => sink.video_sink_pad(),
                Self::Display(sink) => sink.video_sink_pad(),
            }
        }

        /// Get audio sink pad.
        fn audio_sink_pad(&self) -> gst::Pad {
            match self {
                Self::Fake(sink) => sink.audio_sink_pad(),
                Self::Display(sink) => sink.audio_sink_pad(),
            }
        }
    }
}
