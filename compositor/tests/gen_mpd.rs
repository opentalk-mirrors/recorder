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

    let mixer = Mixer::<Grid, TestSource>::new::<DashSink>(resolution, 8, 4).unwrap();

    mixer.play();

    std::thread::sleep(Duration::from_secs(30));
}
