use crate::*;

#[test]
fn test_matroska() {
    // initialize for testing
    testing::init();

    // create grid mixer with test sources for streams and a MatroskaSink
    let mut talk = Talk::<TestSource, u32>::new(
        testing::RESOLUTION,
        MatroskaSink::new("test", Default::default()),
        None,
    )
    .unwrap();

    talk.set_speaker(Some(0), &Default::default()).unwrap();
    // add a stream
    talk.add_stream(
        StreamId::camera(0),
        "Participant 0",
        Default::default(),
        StreamStatus::default(),
    )
    .unwrap();
    talk.layout::<Grid>().unwrap();

    talk.dot("test_matroska", testing::DOT_PARAMS);

    // stir until done
    testing::wait_secs(3);
}
