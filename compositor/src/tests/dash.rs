use super::*;
use crate::*;

#[test]
fn test_dash() {
    // initialize for testing
    testing::init();

    // create grid mixer with test sources for streams and a MatroskaSink
    let mut mixer = Mixer::<TestSource, DashSink, u32>::new(
        testing::RESOLUTION,
        DashParameters {
            output_dir: Some(testing::output_dir().into()),
            seg_duration: 1.0,
            ..Default::default()
        },
    )
    .unwrap();

    // add a stream
    mixer
        .add_stream(0, "Participant 0".into(), Default::default())
        .unwrap();

    // start mixer
    mixer.play();

    mixer.generate_dot_file("test_dash", testing::DOT_DETAILS);

    // stir until done
    testing::wait_secs(5);
}
