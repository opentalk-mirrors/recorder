use crate::*;

use core::time::Duration;
use std::thread::sleep;

#[test]
fn test_speaker_mode() {
    // init logger
    let _ = env_logger::try_init();
    // initialize gstreamer
    gst::init().unwrap();

    let mut mixer = Mixer::<Speaker, TestSource, FakeSink, u32>::new(
        Size {
            width: 640,
            height: 480,
        },
        4,
        (),
        SpeakerMode::FirstShift,
    )
    .unwrap();

    mixer.generate_dot_file("test_speaker_mode-0", gst::DebugGraphDetails::ALL);

    super::generate_participants(&mut mixer, 8);

    mixer.play();

    for i in 0..6 {
        mixer.set_title(&format!("Speaker {i}"));
        mixer.pause();
        mixer.set_speaker(Some(i)).unwrap();
        mixer.play();

        mixer.generate_dot_file(
            &format!("test_speaker_mode-{}", i + 1),
            gst::DebugGraphDetails::ALL,
        );

        sleep(Duration::from_millis(500));
    }
}
