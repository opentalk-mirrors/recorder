use super::*;
use crate::*;

#[test]
fn test_speaker_mode() {
    // init logger
    let _ = env_logger::try_init();
    // initialize gstreamer
    gst::init().unwrap();

    let mut mixer = Mixer::<Speaker, TestSource, TestSink, u32>::new(
        Size {
            width: 640,
            height: 480,
        },
        None,
        (),
        SpeakerMode::FirstShift,
    )
    .unwrap();

    mixer
        .push_overlay(TextOverlay::new("test_speaker_mode", test_name_format()).overlay())
        .unwrap();

    let title = TextOverlay::new("", TextFormat::default());
    mixer.push_overlay(title.overlay()).unwrap();

    mixer.generate_dot_file("test_speaker_mode-0", gst::DebugGraphDetails::ALL);

    generate_streams(&mut mixer, 8);

    mixer.play();

    wait_millis(500);

    for i in 0..6 {
        title.set(&format!("Speaker {i}"));
        mixer.pause();
        mixer.set_speaker(Some(i)).unwrap();
        mixer.play();

        mixer.generate_dot_file(
            &format!("test_speaker_mode-{}", i + 1),
            gst::DebugGraphDetails::ALL,
        );

        wait_millis(500);
    }
}
