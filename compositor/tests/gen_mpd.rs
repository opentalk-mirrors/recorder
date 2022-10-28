use core::time::Duration;

use compositor::*;
use gstreamer as gst;

#[test]
fn generate_mpd() {
    env_logger::init();
    // initialize gstreamer
    gst::init().unwrap();

    // get output resolution from arguments
    let resolution = Size {
        width: 640,
        height: 480,
    };

    let sink_params = DashParameters {
        mpd_root_path: "./tests/output/generate_mpd".into(),
        target_duration: 2,
        ..Default::default()
    };

    let source_params = TestSourceParameters::default();

    if !std::path::Path::new(&sink_params.mpd_root_path).exists() {
        std::fs::create_dir_all(&sink_params.mpd_root_path).unwrap();
    }

    let mut mixer =
        Mixer::<Grid, TestSource>::new::<DashSink>(resolution, 8, 4, sink_params).unwrap();

    mixer
        .add_participant("test".into(), "".into(), source_params)
        .unwrap();
    mixer.play();

    std::thread::sleep(Duration::from_secs(30));

    mixer.play();
}
