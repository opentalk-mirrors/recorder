use super::*;
use crate::*;

#[test]
fn test_overlay() {
    let _ = env_logger::try_init();
    // initialize gstreamer
    gst::init().unwrap();

    // get output resolution from arguments
    let resolution = Size {
        width: 640,
        height: 480,
    };
    let mut mixer = Mixer::<Speaker, TestSource, TestSink, u32>::new(
        resolution,
        Some(6),
        (),
        SpeakerMode::None,
    )
    .unwrap();

    mixer
        .push_overlay(TextOverlay::new("test_overlay", test_name_format()).overlay())
        .unwrap();

    mixer.play();
    mixer.generate_dot_file("test_overlay-0", gst::DebugGraphDetails::ALL);

    let time = 500;
    wait_millis(500);

    // add clock overlay
    mixer.pause();
    mixer
        .push_overlay(ClockOverlay::new(
            "Clock Overlay: %x %X %Z",
            TextFormat::default(),
        ))
        .unwrap();
    mixer.generate_dot_file("test_overlay-1", gst::DebugGraphDetails::ALL);
    mixer.play();

    wait_millis(time);

    // add text overlay
    mixer.pause();
    mixer
        .push_overlay(TextOverlay::new("Text Overlay", TextFormat::default()).into())
        .unwrap();
    mixer.generate_dot_file("test_overlay-2", gst::DebugGraphDetails::ALL);
    mixer.play();

    wait_millis(time);
}
