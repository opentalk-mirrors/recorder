use crate::*;

#[test]
fn test_stream_status() {
    // initialize for testing
    testing::init();

    let mut talk =
        Talk::<TestSource, u32>::new(testing::RESOLUTION, testing::TestSink::default(), None)
            .unwrap();

    talk.dot("test_stream_status-0", testing::DOT_PARAMS);

    testing::generate_streams(&mut talk, 8, 5);

    testing::wait_millis(500);

    for i in 0..5 {
        debug!("Testing stream {i}");

        talk.set_title(&format!("Speaker {i} (audio off)"));
        talk.set_status(
            &StreamId::camera(i),
            StreamStatus {
                has_audio: false,
                has_video: true,
            },
        )
        .unwrap();
        talk.layout::<Speaker>().unwrap();
        talk.dot(
            &format!("test_stream_status-{}-audio-off", i + 1),
            testing::DOT_PARAMS,
        );

        testing::wait();

        talk.set_title(&format!("Speaker {i} (video off)"));
        talk.set_status(
            &StreamId::camera(i),
            StreamStatus {
                has_audio: true,
                has_video: false,
            },
        )
        .unwrap();
        talk.layout::<Speaker>().unwrap();
        talk.dot(
            &format!("test_stream_status-{}-video-off", i + 1),
            testing::DOT_PARAMS,
        );

        testing::wait();

        talk.set_title(&format!("Speaker {i} (a/v off)"));
        talk.set_status(
            &StreamId::camera(i),
            StreamStatus {
                has_audio: false,
                has_video: false,
            },
        )
        .unwrap();
        talk.layout::<Speaker>().unwrap();
        talk.dot(
            &format!("test_stream_status-{}-av-off", i + 1),
            testing::DOT_PARAMS,
        );

        testing::wait();

        talk.set_title(&format!("Speaker {i} (a/v on)"));
        talk.layout::<Speaker>().unwrap();
        talk.set_status(
            &StreamId::camera(i),
            StreamStatus {
                has_audio: true,
                has_video: true,
            },
        )
        .unwrap();
        talk.layout::<Speaker>().unwrap();
        talk.dot(
            &format!("test_stream_status-{}-av-on", i + 1),
            testing::DOT_PARAMS,
        );

        testing::wait();
    }
}
