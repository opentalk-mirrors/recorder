// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use crate::{testing, Mp4Parameters, Mp4Sink, Speaker, StreamId, Talk, TestSource};

#[test]
fn test_mp4() {
    // initialize for testing
    testing::init();
    // create grid mixer with test sources for streams and a MatroskaSink
    let mut mixer = Talk::<TestSource, u32>::new(
        testing::RESOLUTION,
        Speaker::default(),
        testing::MAX_STREAMS,
        true,
    )
    .unwrap();

    mixer
        .link_sink(
            "mp4_sink",
            Mp4Sink::create(
                "test",
                &Mp4Parameters {
                    name: "MP4 Sink",
                    file_path: testing::output_file("mp4sink.mp4").into(),
                },
            )
            .unwrap(),
        )
        .unwrap();

    // add a stream
    mixer
        .add_stream(
            StreamId::camera(0),
            "Participant 0",
            Default::default(),
            Default::default(),
        )
        .unwrap();

    mixer.dot("test_mp4", testing::DOT_PARAMS);

    // stir until done
    testing::wait_secs(10);
}
