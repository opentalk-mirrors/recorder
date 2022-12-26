use crate::*;

#[test]
fn test_overlay() {
    // initialize for testing
    testing::init();

    // get output resolution from arguments
    let mut mixer = Mixer::<Speaker, TestSource, testing::TestSink, u32>::new(
        testing::RESOLUTION,
        Some(6),
        (),
        SpeakerMode::None,
    )
    .unwrap();

    testing::add_overlay_name(&mut mixer, "test_overlay");

    mixer.play();
    mixer.generate_dot_file("test_overlay-0", testing::DOT_DETAILS);

    testing::wait();

    // add clock overlay
    mixer.pause();
    mixer
        .push_overlay(ClockOverlay::new(
            "Clock Overlay: %x %X %Z",
            TextFormat::default(),
        ))
        .unwrap();
    mixer.generate_dot_file("test_overlay-1", testing::DOT_DETAILS);
    mixer.play();

    testing::wait();

    // add text overlay
    mixer.pause();
    mixer
        .push_overlay(TextOverlay::new("Text Overlay", TextFormat::default()).into())
        .unwrap();
    mixer.generate_dot_file("test_overlay-2", testing::DOT_DETAILS);
    mixer.play();

    testing::wait();
}
