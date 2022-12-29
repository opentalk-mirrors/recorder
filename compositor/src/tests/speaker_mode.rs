use crate::*;

#[test]
fn test_speaker_mode() {
    // initialize for testing
    testing::init();

    let mut talk =
        Talk::<TestSource, testing::TestSink, u32>::new(testing::RESOLUTION, (), Some(5)).unwrap();

    // initialize scene
    testing::add_overlay_name(&mut talk.mixer, "test_speaker_mode");
    let title = talk.push_overlay_text("", TextFormat::default()).unwrap();
    let (streams, _) = testing::generate_streams(&mut talk.mixer, 8, 5);

    talk.mixer
        .add_stream(
            StreamId::new(streams[0].0.id, SubStreamId::Screen),
            format!("{}'s screen", streams[0].1),
            TestSourceParameters {
                resolution: Size::SD,
                name: Some(format!("{}'s screen", streams[0].1)),
                pattern: Pattern::Location(testing::image_file("screen_SD.png")),
            },
        )
        .unwrap();

    talk.mixer
        .generate_dot_file("test_speaker_mode-0", testing::DOT_DETAILS);

    talk.play();

    testing::wait();

    for mode in [SpeakerMode::FirstShift, SpeakerMode::FirstSwap] {
        for stream in &streams[0..7] {
            title.set(&format!("Speaker: {} ({mode:?})", stream.1));

            talk.pause();

            talk.set_speaker(Some(stream.0), &mode).unwrap();
            talk.layout::<Speaker>().unwrap();

            talk.play();

            talk.mixer.generate_dot_file(
                &format!("test_speaker_mode-{}-{mode:?}", stream.0.id + 1),
                testing::DOT_DETAILS,
            );

            testing::wait();
        }
    }
}
