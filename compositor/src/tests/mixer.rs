use super::*;
use crate::*;

#[test]
fn test_speaker_layout() {
    test_layout::<Speaker, FakeSink>((), SpeakerMode::FirstShift);
}

#[test]
fn test_grid_layout() {
    test_layout::<Grid, FakeSink>((), SpeakerMode::None);
}

fn test_layout<L, SINK>(params: SINK::Parameters, speaker_mode: SpeakerMode)
where
    L: Layout,
    SINK: Sink,
{
    let _ = env_logger::try_init();

    // initialize gstreamer
    gst::init().unwrap();

    // set output resolution
    let resolution = Size {
        width: 640,
        height: 480,
    };

    let mut mixer =
        Mixer::<L, TestSource, SINK, u32>::new(resolution, None, params, speaker_mode).unwrap();
    mixer.play();
    mixer.generate_dot_file("test_layout-0", gst::DebugGraphDetails::ALL);

    let time = 500;
    let participants = super::generate_ids::<u32>(8);
    let ids: Vec<u32> = participants.iter().map(|p| p.0).collect();

    wait_millis(500);

    mixer.set_subtitle("Sub title");
    mixer.set_title("Add 8 Participants");
    for (id, name) in &participants {
        let params = TestSourceParameters {
            resolution: Size::SD,
            ..Default::default()
        };
        mixer.pause();
        mixer.add_participant(*id, name.clone(), params).unwrap();
        mixer.play();
    }

    for i in 0..6 {
        let j = i + 1;
        mixer.set_title(&format!("Showing {j} Participants"));
        mixer.pause();
        mixer.set_visibles(&ids[0..j]).unwrap();
        mixer.play();

        mixer.generate_dot_file(
            &format!("test_layout-{}-1.{}", L::NAME, j),
            gst::DebugGraphDetails::ALL,
        );
        wait_millis(time);
    }

    for i in 0..6 {
        let j = 6 - i - 1;
        mixer.set_title(&format!("Showing {j} Participants"));
        mixer.pause();
        mixer.set_visibles(&ids[0..j]).unwrap();
        mixer.play();

        mixer.generate_dot_file(
            &format!("test_layout-{}-2.{}", L::NAME, j),
            gst::DebugGraphDetails::ALL,
        );
        wait_millis(time);
    }
}

#[test]
fn test_speaker_different_resolutions() {
    test_layout_different_resolutions::<Speaker, FakeSink>((), SpeakerMode::FirstShift);
}

#[test]
fn test_grid_different_resolutions() {
    test_layout_different_resolutions::<Grid, FakeSink>((), SpeakerMode::None);
}

fn test_layout_different_resolutions<L, SINK>(params: SINK::Parameters, speaker_mode: SpeakerMode)
where
    L: Layout,
    SINK: Sink,
{
    let _ = env_logger::try_init();
    // initialize gstreamer
    gst::init().unwrap();

    // set output resolution
    let resolution = Size::SD;

    let mut mixer =
        Mixer::<L, TestSource, SINK, u32>::new(resolution, None, params, speaker_mode).unwrap();
    mixer.play();
    mixer.generate_dot_file(
        &format!("test_layout_different_resolutions-{}-0", L::NAME),
        gst::DebugGraphDetails::ALL,
    );

    let time = 3000;
    wait_millis(500);
    mixer.pause();

    let (_, ids) = super::generate_participants(&mut mixer, 5);

    mixer.play();

    for i in 1..6 {
        mixer.set_title(&format!("Showing {i} Participants"));
        mixer.pause();
        mixer.set_visibles(&ids[0..i]).unwrap();
        mixer.play();

        mixer.generate_dot_file(
            &format!("test_layout_different_resolutions-{}-1.{i}", L::NAME),
            gst::DebugGraphDetails::ALL,
        );
        wait_millis(time);
    }
    wait_millis(time);
}

#[test]
fn test_remove() {
    let _ = env_logger::try_init();
    // initialize gstreamer
    gst::init().unwrap();

    // get output resolution from arguments
    let resolution = Size {
        width: 640,
        height: 480,
    };
    let mut mixer =
        Mixer::<Grid, TestSource, FakeSink, u32>::new(resolution, Some(6), (), SpeakerMode::None)
            .unwrap();
    mixer.play();

    for i in 0..2 {
        super::generate_participants(&mut mixer, 8);

        mixer.generate_dot_file(&format!("test_remove-{i}-0"), gst::DebugGraphDetails::ALL);

        wait_millis(500);

        mixer.play();
        mixer.generate_dot_file(&format!("test_remove-{i}-1"), gst::DebugGraphDetails::ALL);

        wait_millis(100);

        mixer.set_title("remove 0 (left 1-7)");
        mixer.pause();
        mixer.remove_participant(0).unwrap();

        mixer.play();

        mixer.generate_dot_file(&format!("test_remove-{i}-2"), gst::DebugGraphDetails::ALL);

        wait_millis(100);

        mixer.set_title("remove 1-2 (left 3-7)");
        mixer.pause();
        mixer.remove_participant(1).unwrap();
        mixer.remove_participant(2).unwrap();
        mixer.play();
        mixer.generate_dot_file(&format!("test_remove-{i}-3"), gst::DebugGraphDetails::ALL);

        wait_millis(100);

        mixer.set_title("remove 3-6 (left 7)");
        mixer.pause();
        mixer.remove_participant(3).unwrap();
        mixer.remove_participant(4).unwrap();
        mixer.remove_participant(5).unwrap();
        mixer.remove_participant(6).unwrap();
        mixer.play();
        mixer.generate_dot_file(&format!("test_remove-{i}-4"), gst::DebugGraphDetails::ALL);

        wait_millis(100);

        mixer.set_title("remove 7 (none left)");
        mixer.pause();
        mixer.remove_participant(7).unwrap();
        mixer.play();
        mixer.generate_dot_file(&format!("test_remove-{i}-5"), gst::DebugGraphDetails::ALL);

        // check if we cannot remove any of which we removed before
        assert!(mixer.remove_participant(0).is_err());
        assert!(mixer.remove_participant(1).is_err());
        assert!(mixer.remove_participant(2).is_err());
        assert!(mixer.remove_participant(3).is_err());
        assert!(mixer.remove_participant(4).is_err());
        assert!(mixer.remove_participant(5).is_err());
        assert!(mixer.remove_participant(6).is_err());
        assert!(mixer.remove_participant(7).is_err());

        wait_millis(100);
    }
}
