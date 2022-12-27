use crate::*;

#[test]
fn test_stream_status() {
    // initialize for testing
    testing::init();

    let mut mixer =
        Mixer::<TestSource, testing::TestSink, u32>::new(testing::RESOLUTION, ()).unwrap();

    testing::add_overlay_name(&mut mixer, "test_stream_status");

    let title = TextOverlay::new("", TextFormat::default());
    mixer.push_overlay(title.overlay()).unwrap();

    mixer.generate_dot_file("test_stream_status-0", testing::DOT_DETAILS);

    testing::generate_streams(&mut mixer, 8, 5);

    mixer.play();

    testing::wait_millis(500);

    for i in 0..5 {
        debug!("testing stream {i}");

        title.set(&format!("Speaker {i} (audio off)"));
        mixer.pause();
        mixer
            .set_status(
                i,
                StreamStatus {
                    has_audio: false,
                    has_video: true,
                },
            )
            .unwrap();
        mixer.play();
        mixer.generate_dot_file(
            &format!("test_stream_status-{}-audio-off", i + 1),
            testing::DOT_DETAILS,
        );

        testing::wait();

        title.set(&format!("Speaker {i} (video off)"));
        mixer.pause();
        mixer
            .set_status(
                i,
                StreamStatus {
                    has_audio: true,
                    has_video: false,
                },
            )
            .unwrap();
        mixer.play();
        mixer.generate_dot_file(
            &format!("test_stream_status-{}-video-off", i + 1),
            testing::DOT_DETAILS,
        );

        testing::wait();

        title.set(&format!("Speaker {i} (a/v off)"));
        mixer.pause();
        mixer
            .set_status(
                i,
                StreamStatus {
                    has_audio: false,
                    has_video: false,
                },
            )
            .unwrap();
        mixer.play();
        mixer.generate_dot_file(
            &format!("test_stream_status-{}-av-off", i + 1),
            testing::DOT_DETAILS,
        );

        testing::wait();

        title.set(&format!("Speaker {i} (a/v on)"));
        mixer.pause();
        mixer
            .set_status(
                i,
                StreamStatus {
                    has_audio: true,
                    has_video: true,
                },
            )
            .unwrap();
        mixer.play();
        mixer.generate_dot_file(
            &format!("test_stream_status-{}-av-on", i + 1),
            testing::DOT_DETAILS,
        );

        testing::wait();
    }
}
