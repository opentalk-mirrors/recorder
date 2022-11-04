use super::TEST_OUTPUT_DIR;
use crate::*;
use core::time::Duration;
use gstreamer as gst;

#[test]
fn test_dash() {
    env_logger::init();
    // initialize gstreamer
    gst::init().unwrap();

    // get output resolution from arguments
    let resolution = Size {
        width: 640,
        height: 480,
    };

    // use default parameters for sink
    let sink_params = MatroskaParameters::default();
    let address = sink_params.local_address.clone();
    let port = sink_params.port;

    // create grid mixer with test sources for participants and a MatroskaSink
    let mut mixer =
        Mixer::<Grid, TestSource, MatroskaSink>::new(resolution, 8, 4, sink_params).unwrap();

    // add a participant
    mixer
        .add_participant("test".into(), "".into(), Default::default())
        .unwrap();

    // start mixer
    mixer.play();

    // start ffmpeg to fetch output stream and create DASH files
    std::process::Command::new("ffmpeg")
        .args([
            "-v",
            "warning",
            "-y",
            "-nostdin",
            "-i",
            &format!("tcp://{address}:{port}",),
            "-map",
            "0",
            "-b:0",
            "192k",
            "-use_timeline",
            "1",
            "-use_template",
            "1",
            "-window_size",
            "5",
            "-adaptation_sets",
            "id=0,streams=v id=1,streams=a",
            "-f",
            "dash",
            &format!("{TEST_OUTPUT_DIR}/generate_mpd/output.mpd"),
        ])
        .spawn()
        .unwrap();

    // stir until done
    std::thread::sleep(Duration::from_secs(3));
}
