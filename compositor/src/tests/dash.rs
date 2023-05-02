use super::*;
use crate::*;

#[test]
fn test_dash() {
    // initialize for testing
    testing::init();

    // create grid mixer with test sources for streams and a MatroskaSink
    let mut mixer = Talk::<TestSource, DashSink, u32>::new(
        testing::RESOLUTION,
        DashParameters {
            output_dir: Some(testing::output_dir().into()),
            seg_duration: 1.0,
            ..Default::default()
        },
        None,
    )
    .unwrap();

    // add a stream
    mixer
        .add_stream(
            0.into(),
            "Participant 0".into(),
            Default::default(),
            StreamStatus::default(),
        )
        .unwrap();
    mixer.layout::<Grid>().unwrap();

    mixer.dot("test_dash", testing::DOT_PARAMS);

    // stir until done
    testing::wait_secs(5);
}
