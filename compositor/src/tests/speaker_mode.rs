use crate::*;

#[test]
fn test_speaker_mode() {
    // initialize for testing
    testing::init();

    let mut mixer = Mixer::<Speaker, TestSource, testing::TestSink, u32>::new(
        testing::RESOLUTION,
        None,
        (),
        SpeakerMode::FirstShift,
    )
    .unwrap();

    testing::add_overlay_name(&mut mixer, "test_speaker_mode");

    let title = TextOverlay::new("", TextFormat::default());
    mixer.push_overlay(title.overlay()).unwrap();

    mixer.generate_dot_file("test_speaker_mode-0", testing::DOT_DETAILS);

    testing::generate_streams(&mut mixer, 8);

    mixer.play();

    testing::wait();

    for i in 0..6 {
        title.set(&format!("Speaker {i}"));
        mixer.pause();
        mixer.set_speaker(Some(i)).unwrap();
        mixer.play();

        mixer.generate_dot_file(
            &format!("test_speaker_mode-{}", i + 1),
            testing::DOT_DETAILS,
        );

        testing::wait();
    }
}
