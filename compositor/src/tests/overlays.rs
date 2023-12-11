// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use crate::{testing, Speaker, StreamId, Talk, TestSink, TestSource};

#[test]
fn test_overlay() {
    // initialize for testing
    testing::init();

    // get output resolution from arguments
    let mut talk = Talk::<TestSource, u32>::new(
        testing::RESOLUTION,
        Speaker::default(),
        testing::MAX_STREAMS,
        true,
    )
    .unwrap();
    talk.link_sink("test_sink", TestSink::create("Testing Sink", true).unwrap())
        .unwrap();

    talk.set_speaker(0).unwrap();

    talk.set_title("test_overlay").unwrap();

    talk.dot("test_overlay-0", testing::DOT_PARAMS);

    testing::wait();

    talk.dot("test_overlay-1", testing::DOT_PARAMS);

    testing::wait();

    // add participants
    let (_, ids) = testing::generate_streams(&mut talk, 0, 3, 3, true);
    ids.iter().for_each(|id| {
        talk.show_stream(&StreamId::camera(*id)).unwrap();
    });
    talk.dot("test_overlay-3", testing::DOT_PARAMS);

    testing::wait();

    for id in ids {
        // add text overlay to source
        talk.set_stream_title(&StreamId::camera(id), "new text")
            .unwrap();
        talk.dot("test_overlay-4", testing::DOT_PARAMS);
        testing::wait();
    }

    testing::wait_secs(10);
}
