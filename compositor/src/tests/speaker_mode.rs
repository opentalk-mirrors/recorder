use crate::*;

#[test]
fn test_speaker_mode() {
    // initialize for testing
    testing::init();

    let mut talk =
        Talk::<TestSource, testing::TestSink, u32>::new(testing::RESOLUTION, (), Some(5)).unwrap();

    testing::add_overlay_name(&mut talk.mixer, "test_speaker_mode");

    let title = TextOverlay::new("", TextFormat::default());
    talk.push_overlay(title.overlay()).unwrap();

    talk.mixer
        .generate_dot_file("test_speaker_mode-0", testing::DOT_DETAILS);

    let (streams, _) = testing::generate_streams(&mut talk.mixer, 8, 5);

    talk.play();

    testing::wait();

    for mode in [SpeakerMode::FirstShift, SpeakerMode::FirstSwap] {
        for i in &streams[0..7] {
            title.set(&format!("Speaker: {} ({mode:?})", i.1));
            talk.pause();

            talk.set_speaker(Some(i.0), &mode).unwrap();
            talk.layout::<Speaker>().unwrap();

            talk.play();

            talk.mixer.generate_dot_file(
                &format!("test_speaker_mode-{}-{mode:?}", i.0 + 1),
                testing::DOT_DETAILS,
            );

            testing::wait();
        }
    }
}
