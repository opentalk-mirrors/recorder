use super::*;
use crate::*;

#[test]
fn test_speaker_layout() {
    test_layout::<Speaker>(SpeakerMode::FirstShift);
}

#[test]
fn test_grid_layout() {
    test_layout::<Grid>(SpeakerMode::None);
}

fn test_layout<L>(speaker_mode: SpeakerMode)
where
    L: Layout,
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
        Mixer::<L, TestSource, TestSink, u32>::new(resolution, None, (), speaker_mode).unwrap();

    let title = TextOverlay::new("", &Font::default(), &Padding::default(), &Color::default());
    mixer.push_overlay(title.overlay()).unwrap();

    mixer.play();
    mixer.generate_dot_file("test_layout-0", gst::DebugGraphDetails::ALL);

    let time = 500;
    let streams = super::generate_ids::<u32>(8);
    let ids: Vec<u32> = streams.iter().map(|p| p.0).collect();

    wait_millis(500);

    title.set("Add 8 Participants");
    for (id, name) in &streams {
        let params = TestSourceParameters {
            resolution: Size::SD,
            ..Default::default()
        };
        mixer.pause();
        mixer.add_stream(*id, name.clone(), params).unwrap();
        mixer.play();
    }

    for i in 0..6 {
        let j = i + 1;
        title.set(&format!("Showing {j} Participants"));
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
        title.set(&format!("Showing {j} Participants"));
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
    test_layout_different_resolutions::<Speaker>(SpeakerMode::FirstShift);
}

#[test]
fn test_grid_different_resolutions() {
    test_layout_different_resolutions::<Grid>(SpeakerMode::None);
}

fn test_layout_different_resolutions<L>(speaker_mode: SpeakerMode)
where
    L: Layout,
{
    let _ = env_logger::try_init();
    // initialize gstreamer
    gst::init().unwrap();

    // set output resolution
    let resolution = Size::SD;

    let mut mixer =
        Mixer::<L, TestSource, TestSink, u32>::new(resolution, None, (), speaker_mode).unwrap();

    mixer
        .push_overlay(
            TextOverlay::new(&format!("test_layout_{}", L::NAME), test_name_format()).overlay(),
        )
        .unwrap();

    let title = TextOverlay::new("", TextFormat::default());
    mixer.push_overlay(title.overlay()).unwrap();

    mixer.play();
    mixer.generate_dot_file(
        &format!("test_layout_different_resolutions-{}-0", L::NAME),
        gst::DebugGraphDetails::ALL,
    );

    let time = 3000;
    wait_millis(500);
    mixer.pause();

    let (_, ids) = super::generate_streams(&mut mixer, 5);

    mixer.play();

    for i in 1..6 {
        title.set(&format!("Showing {i} Participants"));
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
        Mixer::<Grid, TestSource, TestSink, u32>::new(resolution, Some(6), (), SpeakerMode::None)
            .unwrap();

    mixer
        .push_overlay(TextOverlay::new("test_remove", test_name_format()).overlay())
        .unwrap();

    let title = TextOverlay::new("", TextFormat::default());
    mixer.push_overlay(title.overlay()).unwrap();

    for i in 0..2 {
        super::generate_streams(&mut mixer, 8);

        mixer.generate_dot_file(&format!("test_remove-{i}-0"), gst::DebugGraphDetails::ALL);

        wait_millis(500);

        mixer.play();
        mixer.generate_dot_file(&format!("test_remove-{i}-1"), gst::DebugGraphDetails::ALL);

        wait_millis(100);

        title.set("remove 0 (left 1-7)");
        mixer.pause();
        mixer.remove_stream(0).unwrap();

        mixer.play();

        mixer.generate_dot_file(&format!("test_remove-{i}-2"), gst::DebugGraphDetails::ALL);

        wait_millis(100);

        title.set("remove 1-2 (left 3-7)");
        mixer.pause();
        mixer.remove_stream(1).unwrap();
        mixer.remove_stream(2).unwrap();
        mixer.play();
        mixer.generate_dot_file(&format!("test_remove-{i}-3"), gst::DebugGraphDetails::ALL);

        wait_millis(100);

        title.set("remove 3-6 (left 7)");
        mixer.pause();
        mixer.remove_stream(3).unwrap();
        mixer.remove_stream(4).unwrap();
        mixer.remove_stream(5).unwrap();
        mixer.remove_stream(6).unwrap();
        mixer.play();
        mixer.generate_dot_file(&format!("test_remove-{i}-4"), gst::DebugGraphDetails::ALL);

        wait_millis(100);

        title.set("remove 7 (none left)");
        mixer.pause();
        mixer.remove_stream(7).unwrap();
        mixer.play();
        mixer.generate_dot_file(&format!("test_remove-{i}-5"), gst::DebugGraphDetails::ALL);

        // check if we cannot remove any of which we removed before
        assert!(mixer.remove_stream(0).is_err());
        assert!(mixer.remove_stream(1).is_err());
        assert!(mixer.remove_stream(2).is_err());
        assert!(mixer.remove_stream(3).is_err());
        assert!(mixer.remove_stream(4).is_err());
        assert!(mixer.remove_stream(5).is_err());
        assert!(mixer.remove_stream(6).is_err());
        assert!(mixer.remove_stream(7).is_err());

        wait_millis(100);
    }
}
