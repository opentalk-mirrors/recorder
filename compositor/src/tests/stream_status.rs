// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use types::signaling::media::MediaSessionState;

use crate::{testing, Mixer, Speaker, StreamId, TestSink, TestSource};

#[test]
fn test_stream_status() {
    // initialize for testing
    testing::init();

    let mut mixer = Mixer::<TestSource, u32>::new(
        testing::RESOLUTION,
        Speaker::default(),
        testing::MAX_STREAMS,
        true,
    )
    .unwrap();
    mixer
        .link_sink("test_sink", TestSink::create("Testing Sink", true).unwrap())
        .unwrap();

    mixer.dot("test_stream_status-0", testing::DOT_PARAMS);

    testing::generate_streams(&mut mixer, 0, 8, 5, true);

    testing::wait_millis(500);

    for i in 0..5 {
        debug!("Testing stream {i}");

        mixer
            .set_title(&format!("Speaker {i} (audio off)"))
            .unwrap();
        mixer
            .set_status(
                &StreamId::camera(i),
                &MediaSessionState {
                    audio: false,
                    video: true,
                },
            )
            .unwrap();
        mixer.dot(
            &format!("test_stream_status-{}-audio-off", i + 1),
            testing::DOT_PARAMS,
        );

        testing::wait();

        mixer
            .set_title(&format!("Speaker {i} (video off)"))
            .unwrap();
        mixer
            .set_status(
                &StreamId::camera(i),
                &MediaSessionState {
                    audio: true,
                    video: false,
                },
            )
            .unwrap();
        mixer.dot(
            &format!("test_stream_status-{}-video-off", i + 1),
            testing::DOT_PARAMS,
        );

        testing::wait();

        mixer.set_title(&format!("Speaker {i} (a/v off)")).unwrap();
        mixer
            .set_status(
                &StreamId::camera(i),
                &MediaSessionState {
                    audio: false,
                    video: false,
                },
            )
            .unwrap();
        mixer.dot(
            &format!("test_stream_status-{}-av-off", i + 1),
            testing::DOT_PARAMS,
        );

        testing::wait();

        mixer.set_title(&format!("Speaker {i} (a/v on)")).unwrap();
        mixer
            .set_status(
                &StreamId::camera(i),
                &MediaSessionState {
                    audio: true,
                    video: true,
                },
            )
            .unwrap();
        mixer.dot(
            &format!("test_stream_status-{}-av-on", i + 1),
            testing::DOT_PARAMS,
        );

        testing::wait();
    }
}
