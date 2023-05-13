use crate::*;

#[test]
fn test_matroska() {
    // initialize for testing
    testing::init();

    // create grid mixer with test sources for streams and a MatroskaSink
    let mut mixer = Talk::<TestSource, u32>::new(
        testing::RESOLUTION,
        Box::new(MatroskaSinkBuilder::new(Default::default())),
        None,
    )
    .unwrap();

    // add a stream
    mixer
        .add_stream(
            StreamId::camera(0),
            "Participant 0",
            Default::default(),
            StreamStatus::default(),
        )
        .unwrap();

    mixer.dot("test_matroska", testing::DOT_PARAMS);

    // stir until done
    testing::wait_secs(3);
}
