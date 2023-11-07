// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use crate::*;

#[test]
fn test_overlay() {
    // initialize for testing
    testing::init();

    // get output resolution from arguments
    let mut talk = Talk::<TestSource, u32>::new(
        testing::RESOLUTION,
        Speaker::default(),
        TestSink::default(),
        testing::MAX_STREAMS,
    )
    .unwrap();

    talk.set_speaker(0);

    talk.set_title("test_overlay");

    talk.dot("test_overlay-0", testing::DOT_PARAMS);

    testing::wait();

    talk.dot("test_overlay-1", testing::DOT_PARAMS);

    testing::wait();

    // add participants
    let (_, ids) = testing::generate_streams(&mut talk, 0, 3, 3);
    ids.iter().for_each(|id| {
        talk.show_stream(&StreamId::camera(*id));
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
}
