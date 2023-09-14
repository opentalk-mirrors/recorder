use crate::*;

#[test]
fn test_speaker_mode() {
    // initialize for testing
    testing::init();

    const MAX_VISIBLES: usize = 5;
    const NUM_PARTICIPANTS: usize = 10;

    let mut talk = Talk::<TestSource, u32>::new(
        testing::RESOLUTION,
        testing::TestSink::default(),
        Some(MAX_VISIBLES),
    )
    .unwrap();

    // initialize scene
    talk.set_title("test_speaker_mode");

    let (streams, _) = testing::generate_streams(&mut talk, NUM_PARTICIPANTS as u32, MAX_VISIBLES);

    talk.add_stream(
        StreamId::screen(streams[0].0),
        &format!("{}'s screen", streams[0].1),
        TestSourceParameters {
            resolution: Size::SD,
            name: Some(format!("{}'s screen", streams[0].1)),
            pattern: Pattern::Smpte75,
        },
        StreamStatus::default(),
    )
    .unwrap();

    talk.dot("test_speaker_mode-0", testing::DOT_PARAMS);

    testing::wait();

    for mode in [SpeakerSwitchMode::FirstShift, SpeakerSwitchMode::FirstSwap] {
        for stream in &streams[0..NUM_PARTICIPANTS] {
            talk.set_title(&format!("Speaker: {} ({mode:?})", stream.1));

            talk.set_speaker(Some(stream.0), &mode).unwrap();
            talk.layout::<Speaker>().unwrap();

            talk.dot(
                &format!("test_speaker_mode-{}-{mode:?}", stream.0 + 1),
                testing::DOT_PARAMS,
            );

            testing::wait();
        }
    }
}
