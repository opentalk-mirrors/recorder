// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use crate::*;

#[test]
fn test_multi() {
    // initialize for testing
    testing::init();
    // create grid mixer with test sources for streams and a MatroskaSink
    let mut talk = Talk::<TestSource, u32>::new(
        testing::RESOLUTION,
        Speaker::default(),
        MultiSink::create(MultiParameters {
            sinks: vec![
                Box::new(TestSink::create("Sink 1").unwrap()),
                Box::new(TestSink::create("Sink 2").unwrap()),
            ],
        })
        .unwrap(),
        testing::MAX_STREAMS,
    )
    .unwrap();

    testing::generate_streams(&mut talk, 0, 3, 3);
    talk.set_speaker(0).unwrap();

    talk.dot("test_multi", testing::DOT_PARAMS);

    // stir until done
    testing::wait_secs(4);
}
