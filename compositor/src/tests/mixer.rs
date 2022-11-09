use crate::*;
use std::thread::sleep;
use std::time::Duration;

fn generate_ids(count: u32) -> Vec<u32> {
    (0..count).collect()
}

#[test]
fn test_speaker() {
    test_layout::<Speaker>();
}

#[test]
fn test_grid() {
    test_layout::<Grid>();
}

fn test_layout<L>()
where
    L: Layout,
{
    let _ = env_logger::try_init();

    // initialize gstreamer
    gst::init().unwrap();

    // get output resolution from arguments
    let resolution = Size {
        width: 640,
        height: 480,
    };

    let mut mixer = Mixer::<L, TestSource, DisplaySink, u32>::new(resolution, 6, ()).unwrap();
    mixer.play();

    mixer.generate_dot_file("test_layout-0", gst::DebugGraphDetails::ALL);

    let time = 500;

    let ids = generate_ids(8);

    sleep(Duration::from_millis(500));

    mixer.set_speaking("Speaking");

    mixer.set_title("Add 8 Participants");
    mixer.pause();
    for &id in &ids {
        let params = TestSourceParameters {
            resolution: Size::SD,
            ..Default::default()
        };
        mixer
            .add_participant(id, format!("Participant {id}"), params)
            .unwrap();
    }

    for i in 0..6 {
        let j = i + 1;
        mixer.set_title(&format!("Showing {j} Participants"));
        mixer.pause();
        mixer.set_visibles(&ids[0..j]).unwrap();
        mixer.play();

        mixer.generate_dot_file(
            &format!("test_layout-{}-1.{}", L::name(), j),
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
            &format!("test_layout-{}-2.{}", L::name(), j),
            gst::DebugGraphDetails::ALL,
        );
        sleep(Duration::from_millis(time));
    }
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
    let mut mixer = Mixer::<Grid, TestSource, FakeSink, u32>::new(resolution, 4, ()).unwrap();
    mixer.play();

    mixer.generate_dot_file("test_remove-0", gst::DebugGraphDetails::ALL);

    let ids = generate_ids(8);

    sleep(Duration::from_millis(500));

    mixer.set_title("add 8 participants");
    mixer.pause();
    for &id in &ids {
        let params = TestSourceParameters {
            resolution: Size::FHD,
            ..Default::default()
        };
        mixer
            .add_participant(id, format!("Participant {id}"), params)
            .unwrap();
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
    for &id in &ids[0..2] {
        mixer.remove_participant(id).unwrap();
    }
    mixer.play();
    mixer.generate_dot_file("test_remove-3", gst::DebugGraphDetails::ALL);

    sleep(Duration::from_secs(1));

    mixer.set_title("show 2-6");
    mixer.pause();
    for &id in &ids[4..8] {
        mixer.remove_participant(id).unwrap();
    }
    mixer.play();
    mixer.generate_dot_file("test_remove-4", gst::DebugGraphDetails::ALL);

    sleep(Duration::from_secs(1));
}
