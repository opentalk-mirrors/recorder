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
                Box::new(TestSink::new("Sink 1")),
                Box::new(TestSink::new("Sink 2")),
            ],
        }),
        testing::MAX_STREAMS,
    )
    .unwrap();

    testing::generate_streams(&mut talk, 0, 3, 3);
    talk.set_speaker(0);
    talk.layout::<Grid>().unwrap();

    talk.dot("test_multi", testing::DOT_PARAMS);

    // stir until done
    testing::wait_secs(4);
}
