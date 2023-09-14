use crate::*;

#[test]
fn test_multi() {
    // initialize for testing
    testing::init();
    // create grid mixer with test sources for streams and a MatroskaSink
    let mut talk = Talk::<TestSource, u32>::new(
        testing::RESOLUTION,
        MultiSink::new(MultiParameters {
            sinks: vec![
                Box::new(testing::TestSink::new("Sink 1")),
                Box::new(testing::TestSink::new("Sink 2")),
            ],
        }),
        None,
    )
    .unwrap();

    testing::generate_streams(&mut talk, 3, 3);
    talk.set_speaker(Some(0), &SpeakerSwitchMode::FirstShift)
        .unwrap();
    talk.layout::<Grid>().unwrap();

    talk.dot("test_multi", testing::DOT_PARAMS);

    // stir until done
    testing::wait_secs(4);
}
