use crate::*;
use std::thread::sleep;
use std::time::Duration;

#[test]
fn test_speaker() {
    test_layout::<Speaker, FakeSink>((), SpeakerMode::FirstShift);
}

#[test]
fn test_grid() {
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
        Mixer::<L, TestSource, SINK, u32>::new(resolution, 6, 8, params, speaker_mode).unwrap();
    mixer.play();
    mixer.generate_dot_file("test_layout-0", gst::DebugGraphDetails::ALL);

    let time = 500;
    let participants = super::generate_ids::<u32>(8);
    let ids: Vec<u32> = participants.iter().map(|p| p.0).collect();

    sleep(Duration::from_millis(500));

    mixer.set_subtitle("Sub title");
    mixer.set_title("Add 8 Participants");
    mixer.pause();
    for (id, name) in &participants {
        let params = TestSourceParameters {
            resolution: Size::SD,
            ..Default::default()
        };
        mixer.add_participant(*id, name.clone(), params).unwrap();
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
        sleep(Duration::from_millis(time));
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
        sleep(Duration::from_millis(time));
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
        Mixer::<L, TestSource, SINK, u32>::new(resolution, 5, 5, params, speaker_mode).unwrap();
    mixer.play();
    mixer.generate_dot_file(
        &format!("test_layout_different_resolutions-{}-0", L::NAME),
        gst::DebugGraphDetails::ALL,
    );

    let time = 3000;

    sleep(Duration::from_millis(500));
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
        sleep(Duration::from_millis(time));
    }
    sleep(Duration::from_millis(time));
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
        Mixer::<Grid, TestSource, FakeSink, u32>::new(resolution, 4, 8, (), SpeakerMode::None)
            .unwrap();
    mixer.play();

    mixer.generate_dot_file("test_remove-0", gst::DebugGraphDetails::ALL);

    let participants = super::generate_ids(8);
    let ids: Vec<u32> = participants.iter().map(|p| p.0).collect();

    sleep(Duration::from_millis(500));

    mixer.set_title("add 8 participants");
    mixer.pause();
    for (id, name) in &participants {
        let params = TestSourceParameters {
            resolution: Size::FHD,
            ..Default::default()
        };
        mixer.add_participant(*id, name.into(), params).unwrap();
    }
    mixer.play();
    mixer.generate_dot_file("test_remove-1", gst::DebugGraphDetails::ALL);

    sleep(Duration::from_millis(500));

    mixer.set_title("show 1-4");
    mixer.pause();
    mixer.set_visibles(&ids[0..4]).unwrap();
    mixer.play();

    mixer.generate_dot_file("test_remove-2", gst::DebugGraphDetails::ALL);

    sleep(Duration::from_secs(1));

    mixer.set_title("remove 1-2");
    mixer.pause();
    for (id, _) in &participants[0..2] {
        mixer.remove_participant(*id).unwrap();
    }
    mixer.play();
    mixer.generate_dot_file("test_remove-3", gst::DebugGraphDetails::ALL);

    sleep(Duration::from_secs(1));

    mixer.set_title("show 2-6");
    mixer.pause();
    for (id, _) in &participants[4..8] {
        mixer.remove_participant(*id).unwrap();
    }
    mixer.play();
    mixer.generate_dot_file("test_remove-4", gst::DebugGraphDetails::ALL);

    sleep(Duration::from_secs(1));
}
