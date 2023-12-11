// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use crate::{testing, DashParameters, DashSink, Speaker, StreamId, StreamStatus, Talk, TestSource};

#[test]
fn test_dash() {
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
        "dash_sink",
        DashSink::create(
            "test",
            DashParameters {
                output_dir: Some(testing::output_dir().into()),
                seg_duration: 1.0,
                ..Default::default()
            },
        )
        .unwrap(),
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

    talk.dot("test_dash", testing::DOT_PARAMS);

    // stir until done
    testing::wait_secs(10);
}
