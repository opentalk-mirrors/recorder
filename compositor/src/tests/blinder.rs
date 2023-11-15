// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use crate::*;

#[test]
fn test_blinder() {
    // initialize for testing
    testing::init();

    let blinder = TestBlinder::new(&TestBlinderParams {
        name: "Testing Blinder",
        sink: Box::new(TestSink::new("Testing Sink").unwrap()),
        resolution: testing::RESOLUTION,
        alt_source_params: TestSourceParameters::default(),
    })
    .unwrap();
    let mut talk = Talk::<TestSource, u32>::new(
        testing::RESOLUTION,
        Speaker::default(),
        blinder.clone(),
        testing::MAX_STREAMS,
    )
    .unwrap();

    testing::generate_streams(&mut talk, 0, 8, 5);
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
