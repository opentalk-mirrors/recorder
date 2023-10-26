use crate::*;

#[test]
fn test_dash() {
    // initialize for testing
    testing::init();

    // create grid mixer with test sources for streams and a MatroskaSink
    let mut talk = Talk::<TestSource, u32>::new(
        testing::RESOLUTION,
        DashSink::new(
            "test",
            DashParameters {
                output_dir: Some(testing::output_dir().into()),
                seg_duration: 1.0,
                ..Default::default()
            },
        ),
        testing::MAX_STREAMS,
    )
    .unwrap();

    talk.set_speaker(0);
    // add a stream
    talk.add_stream(
        StreamId::camera(0),
        "Participant 0",
        Default::default(),
        StreamStatus::default(),
    )
    .unwrap();
    talk.layout::<Grid>().unwrap();

    talk.dot("test_dash", testing::DOT_PARAMS);

    // stir until done
    testing::wait_secs(5);
}
