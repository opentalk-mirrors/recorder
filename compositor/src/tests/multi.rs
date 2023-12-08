// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use crate::{testing, Speaker, Talk, TestSink, TestSource};

#[test]
fn test_multi() {
    // initialize for testing
    testing::init();
    // create grid mixer with test sources for streams and a MatroskaSink
    let mut talk = Talk::<TestSource, u32>::new(
        testing::RESOLUTION,
        Speaker::default(),
        vec![
            Box::new(TestSink::create("Sink 0").unwrap()),
            Box::new(TestSink::create("Sink 1").unwrap()),
            Box::new(TestSink::create("Sink 2").unwrap()),
            Box::new(TestSink::create("Sink 3").unwrap()),
            Box::new(TestSink::create("Sink 4").unwrap()),
            Box::new(TestSink::create("Sink 5").unwrap()),
            Box::new(TestSink::create("Sink 6").unwrap()),
            Box::new(TestSink::create("Sink 7").unwrap()),
            Box::new(TestSink::create("Sink 8").unwrap()),
            Box::new(TestSink::create("Sink 9").unwrap()),
        ],
        testing::MAX_STREAMS,
    )
    .unwrap();

    testing::generate_streams(&mut talk, 0, 3, 3);
    talk.set_speaker(0).unwrap();

    talk.dot("test_multi", testing::DOT_PARAMS);

    // stir until done
    testing::wait_secs(4);
}
