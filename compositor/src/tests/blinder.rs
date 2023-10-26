use crate::*;

#[test]
fn test_blinder() {
    // initialize for testing
    testing::init();

    let blinder = TestBlinder::new(TestBlinderParams {
        sink: Box::new(TestSink::new("Testing Sink")),
        resolution: testing::RESOLUTION,
        ..Default::default()
    });
    let mut talk =
        Talk::<TestSource, u32>::new(testing::RESOLUTION, blinder.clone(), testing::MAX_STREAMS)
            .unwrap();

    testing::generate_streams(&mut talk, 0, 8, 5);
    talk.set_speaker(0);
    talk.layout::<Grid>().unwrap();
    blinder.blind(false);

    talk.set_title("not blinded");
    talk.dot("test_blinder-not_blinded", testing::DOT_PARAMS);
    testing::wait();

    blinder.blind(true);

    talk.set_title("blinded");
    talk.dot("test_blinder-blinded", testing::DOT_PARAMS);
    testing::wait();

    blinder.blind(false);

    talk.set_title("not blinded");
    talk.dot("test_blinder-not_blinded", testing::DOT_PARAMS);
    testing::wait();
    talk.set_title("shutdown");
}
