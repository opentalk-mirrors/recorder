use crate::*;

#[test]
fn test_matroska() {
    // initialize for testing
    testing::init();

    // create grid mixer with test sources for streams and a MatroskaSink
    let mut mixer = Mixer::<Grid, TestSource, MatroskaSink, u32>::new(
        testing::RESOLUTION,
        None,
        MatroskaParameters::default(),
        SpeakerMode::None,
    )
    .unwrap();

    // add a stream
    mixer
        .add_stream(0, "Participant 0".into(), Default::default())
        .unwrap();

    // start mixer
    mixer.play();

    mixer.generate_dot_file("test_matroska", testing::DOT_DETAILS);

    // stir until done
    testing::wait_secs(3);
}
