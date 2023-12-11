// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use crate::{testing, Speaker, Talk, TestSink, TestSource};

fn test_multi(use_video: bool) {
    // initialize for testing
    testing::init();
    // create grid mixer with test sources for streams and a MatroskaSink
    let mut talk = Talk::<TestSource, u32>::new(
        testing::RESOLUTION,
        Speaker::default(),
        testing::MAX_STREAMS,
        use_video,
    )
    .unwrap();

    for i in 0..10 {
        let name = format!("sink_{i}");
        let sink = TestSink::create(&name, use_video).unwrap();
        talk.link_sink(&name, sink).unwrap();
    }

    testing::generate_streams(&mut talk, 0, 3, 3, use_video);
    talk.set_speaker(0).unwrap();

    talk.dot("test_multi", testing::DOT_PARAMS);

    // stir until done
    testing::wait_secs(4);
}

#[test]
fn test_multi_audio() {
    test_multi(false);
}

#[test]
fn test_multi_video() {
    test_multi(true);
}
