// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use crate::{
    testing, Blinder, Speaker, Talk, TestBlinder, TestBlinderParams, TestSink, TestSource,
    TestSourceParameters,
};

#[test]
fn test_blinder() {
    // initialize for testing
    testing::init();

    let blinder = TestBlinder::create(&TestBlinderParams {
        name: "Testing Blinder",
        sink: Box::new(TestSink::create("Testing Sink", true).unwrap()),
        resolution: testing::RESOLUTION,
        alt_source_params: TestSourceParameters::default(),
    })
    .unwrap();
    let mut talk = Talk::<TestSource, u32>::new(
        testing::RESOLUTION,
        Speaker::default(),
        testing::MAX_STREAMS,
        true,
    )
    .unwrap();

    talk.link_sink("blinder", blinder.clone()).unwrap();

    testing::generate_streams(&mut talk, 0, 8, 5, true);
    talk.set_speaker(0).unwrap();
    blinder.blind(false);

    talk.set_title("not blinded").unwrap();
    talk.dot("test_blinder-not_blinded", testing::DOT_PARAMS);
    testing::wait();

    blinder.blind(true);

    talk.set_title("blinded").unwrap();
    talk.dot("test_blinder-blinded", testing::DOT_PARAMS);
    testing::wait();

    blinder.blind(false);

    talk.set_title("not blinded").unwrap();
    talk.dot("test_blinder-not_blinded", testing::DOT_PARAMS);
    testing::wait();
    talk.set_title("shutdown").unwrap();
}
