use super::*;
use crate::*;

#[test]
fn test_participant_status() {
    // init logger
    let _ = env_logger::try_init();
    // initialize gstreamer
    gst::init().unwrap();

    let mut mixer = Mixer::<Speaker, TestSource, FakeSink, u32>::new(
        Size {
            width: 640,
            height: 480,
        },
        Some(5),
        (),
        SpeakerMode::FirstShift,
    )
    .unwrap();

    mixer.generate_dot_file("test_participant_status-0", gst::DebugGraphDetails::ALL);

    trace!("Mixer State: {:?}", mixer.state());
    generate_participants(&mut mixer, 8);

    mixer.play();

    wait_millis(500);

    for i in 0..5 {
        debug!("testing participant {i}");

        mixer.set_title(&format!("Speaker {i} (audio off)"));
        mixer.pause();
        mixer
            .set_status(
                i,
                ParticipantStatus {
                    has_audio: false,
                    has_video: true,
                },
            )
            .unwrap();
        mixer.play();
        mixer.generate_dot_file(
            &format!("test_participant_status-{}-audio-off", i + 1),
            gst::DebugGraphDetails::ALL,
        );
        wait_millis(500);

        mixer.set_title(&format!("Speaker {i} (video off)"));
        mixer.pause();
        mixer
            .set_status(
                i,
                ParticipantStatus {
                    has_audio: true,
                    has_video: false,
                },
            )
            .unwrap();
        mixer.play();
        mixer.generate_dot_file(
            &format!("test_participant_status-{}-video-off", i + 1),
            gst::DebugGraphDetails::ALL,
        );
        wait_millis(500);

        mixer.set_title(&format!("Speaker {i} (a/v off)"));
        mixer.pause();
        mixer
            .set_status(
                i,
                ParticipantStatus {
                    has_audio: false,
                    has_video: false,
                },
            )
            .unwrap();
        mixer.play();
        mixer.generate_dot_file(
            &format!("test_participant_status-{}-av-off", i + 1),
            gst::DebugGraphDetails::ALL,
        );
        wait_millis(500);

        mixer.set_title(&format!("Speaker {i} (a/v on)"));
        mixer.pause();
        mixer
            .set_status(
                i,
                ParticipantStatus {
                    has_audio: true,
                    has_video: true,
                },
            )
            .unwrap();
        mixer.play();
        mixer.generate_dot_file(
            &format!("test_participant_status-{}-av-on", i + 1),
            gst::DebugGraphDetails::ALL,
        );
        wait_millis(500);
    }
}
