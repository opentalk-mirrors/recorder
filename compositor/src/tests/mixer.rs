use crate::*;
use gstreamer as gst;
use std::thread::sleep;
use std::time::Duration;

fn generate_ids(count: usize) -> Vec<String> {
    // add participant names
    (0..count).map(|n| format!("Participant {n}")).collect()
}

#[test]
fn test_visibles() {
    env_logger::init();
    // initialize gstreamer
    gst::init().unwrap();

    // get output resolution from arguments
    let resolution = Size {
        width: 640,
        height: 480,
    };
    let mut mixer = Mixer::<Grid, TestSource, DisplaySink>::new(resolution, 4, ()).unwrap();
    mixer.play();

    mixer.generate_dot_file("test_visible-0.dot", gst::DebugGraphDetails::ALL);

    let ids = generate_ids(8);

    sleep(Duration::from_millis(500));

    mixer.set_title("add 8 participants");
    mixer.pause();
    for id in &ids {
        let params = TestSourceParameters {
            resolution: Size::FHD,
            ..Default::default()
        };
        mixer
            .add_participant(id.clone(), id.clone(), params)
            .unwrap();
    }
    mixer.play();
    mixer.generate_dot_file("test_visible-1.dot", gst::DebugGraphDetails::ALL);

    sleep(Duration::from_millis(500));
    mixer.set_title("show 1-4");
    mixer.pause();
    mixer.set_visibles(&ids[0..4]).unwrap();
    mixer.play();

    mixer.generate_dot_file("test_visible-2.dot", gst::DebugGraphDetails::ALL);

    sleep(Duration::from_millis(500));

    mixer.set_title("show 5-6");
    mixer.pause();
    mixer.set_visibles(&ids[4..6]).unwrap();
    mixer.play();
    mixer.generate_dot_file("test_visible-3.dot", gst::DebugGraphDetails::ALL);

    mixer.set_title("show 6-8");
    mixer.pause();
    mixer.set_visibles(&ids[5..8]).unwrap();
    mixer.play();
    mixer.generate_dot_file("test_visible-4.dot", gst::DebugGraphDetails::ALL);

    mixer.set_title("show 8");
    mixer.pause();
    mixer.set_visibles(&ids[7..8]).unwrap();
    mixer.play();
    mixer.generate_dot_file("test_visible-5.dot", gst::DebugGraphDetails::ALL);

    sleep(Duration::from_millis(500));
}

#[test]
fn test_remove() {
    env_logger::init();
    // initialize gstreamer
    gst::init().unwrap();

    // get output resolution from arguments
    let resolution = Size {
        width: 640,
        height: 480,
    };
    let mut mixer = Mixer::<Grid, TestSource, FakeSink>::new(resolution, 4, ()).unwrap();
    mixer.play();

    mixer.generate_dot_file("test_visible-0.dot", gst::DebugGraphDetails::ALL);

    let ids = generate_ids(8);

    sleep(Duration::from_millis(500));

    mixer.set_title("add 8 participants");
    mixer.pause();
    for id in &ids {
        let params = TestSourceParameters {
            resolution: Size::FHD,
            ..Default::default()
        };
        mixer
            .add_participant(id.clone(), id.into(), params)
            .unwrap();
    }
    mixer.play();
    mixer.generate_dot_file("test_visible-1.dot", gst::DebugGraphDetails::ALL);

    sleep(Duration::from_millis(500));

    mixer.set_title("show 1-4");
    mixer.pause();
    mixer.set_visibles(&ids[0..4]).unwrap();
    mixer.play();

    mixer.generate_dot_file("test_visible-2.dot", gst::DebugGraphDetails::ALL);

    sleep(Duration::from_secs(1));

    mixer.set_title("remove 1-2");
    mixer.pause();
    for name in &ids[0..2] {
        mixer.remove_participant(name).unwrap();
    }
    mixer.play();
    mixer.generate_dot_file("test_visible-3.dot", gst::DebugGraphDetails::ALL);

    sleep(Duration::from_secs(1));

    mixer.set_title("show 2-6");
    mixer.pause();
    for name in &ids[4..8] {
        mixer.remove_participant(name).unwrap();
    }
    mixer.play();
    mixer.generate_dot_file("test_visible-4.dot", gst::DebugGraphDetails::ALL);

    sleep(Duration::from_secs(1));
}
