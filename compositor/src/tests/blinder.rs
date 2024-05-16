// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use types::core::ParticipantId;

use crate::{
    testing, Blinder, Mixer, Speaker, TestBlinder, TestBlinderParams, TestSink, TestSource,
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
    let mut mixer = Mixer::<TestSource>::create(
        None,
        testing::RESOLUTION,
        Speaker::default(),
        testing::MAX_STREAMS,
        true,
        &Default::default(),
    )
    .unwrap();

    mixer.link_sink("blinder", blinder.clone()).unwrap();

    testing::generate_streams(&mut mixer, 0, 8, 5, true);
    mixer.set_speaker(ParticipantId::from_u128(0)).unwrap();
    blinder.blind(false);

    mixer.set_title("not blinded");
    mixer.dot("test_blinder-not_blinded", testing::DOT_PARAMS);
    testing::wait();

    blinder.blind(true);

    mixer.set_title("blinded");
    mixer.dot("test_blinder-blinded", testing::DOT_PARAMS);
    testing::wait();

    blinder.blind(false);

    mixer.set_title("not blinded");
    mixer.dot("test_blinder-not_blinded", testing::DOT_PARAMS);
    testing::wait();
    mixer.set_title("shutdown");
}
