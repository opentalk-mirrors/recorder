use crate::*;

#[test]
fn test_stream_status() {
    // initialize for testing
    testing::init();

    let mut mixer =
        Talk::<TestSource, testing::TestSink, u32>::new(testing::RESOLUTION, (), None).unwrap();

    testing::add_overlay_name(&mut mixer, "test_stream_status");

    let title = mixer
        .insert_overlay_text("", TextFormat::default())
        .unwrap();

    mixer.dot("test_stream_status-0", testing::DOT_PARAMS);

    testing::generate_streams(&mut mixer, 8, 5);

    testing::wait_millis(500);

    for i in 0..5 {
        debug!("Testing stream {i}");

        title.set(&format!("Speaker {i} (audio off)"));
        mixer
            .set_status(
                &i.into(),
                StreamStatus {
                    has_audio: false,
                    has_video: true,
                },
            )
            .unwrap();
        mixer.dot(
            &format!("test_stream_status-{}-audio-off", i + 1),
            testing::DOT_PARAMS,
        );

        testing::wait();

        title.set(&format!("Speaker {i} (video off)"));
        mixer
            .set_status(
                &i.into(),
                StreamStatus {
                    has_audio: true,
                    has_video: false,
                },
            )
            .unwrap();
        mixer.dot(
            &format!("test_stream_status-{}-video-off", i + 1),
            testing::DOT_PARAMS,
        );

        testing::wait();

        title.set(&format!("Speaker {i} (a/v off)"));
        mixer
            .set_status(
                &i.into(),
                StreamStatus {
                    has_audio: false,
                    has_video: false,
                },
            )
            .unwrap();
        mixer.dot(
            &format!("test_stream_status-{}-av-off", i + 1),
            testing::DOT_PARAMS,
        );

        testing::wait();

        title.set(&format!("Speaker {i} (a/v on)"));
        mixer
            .set_status(
                &i.into(),
                StreamStatus {
                    has_audio: true,
                    has_video: true,
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
