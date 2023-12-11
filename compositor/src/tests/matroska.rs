// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use crate::{testing, MatroskaSink, Speaker, StreamId, StreamStatus, Talk, TestSource};

#[test]
fn test_matroska() {
    // initialize for testing
    testing::init();

    // create grid mixer with test sources for streams and a MatroskaSink
    let mut talk = Talk::<TestSource, u32>::new(
        testing::RESOLUTION,
        Speaker::default(),
        testing::MAX_STREAMS,
        true,
    )
    .unwrap();

    talk.link_sink(
        "matroska_sink",
        MatroskaSink::create("test", &Default::default()).unwrap(),
    )
    .unwrap();

    talk.set_speaker(0).unwrap();
    // add a stream
    talk.add_stream(
        StreamId::camera(0),
        "Participant 0",
        Default::default(),
        StreamStatus::default(),
    )
    .unwrap();

    talk.dot("test_matroska", testing::DOT_PARAMS);

    // stir until done
    testing::wait_secs(3);
}
