extern crate clap;

use compositor::*;
use gstreamer as gst;
use std::thread::sleep;
use std::time::Duration;

fn names(count: usize) -> Vec<String> {
    // add participant names
    (0..count).map(|n| format!("Participant {n}")).collect()
}

#[test]
fn test_visibles() {
    // initialize gstreamer
    gst::init().unwrap();

    // get output resolution from arguments
    let resolution = Size {
        width: 640,
        height: 480,
    };
    let mut mixer = Mixer::<Grid, TestSource>::new::<DisplaySink>(resolution, 8, 4).unwrap();
    mixer.play();

    mixer.generate_dot_file("test_visible-0.dot");

    let names = names(8);

    sleep(Duration::from_millis(500));

    mixer.set_title("add 8 participants");
    mixer.pause();
    for name in &names {
        mixer
            .add_participant(name.clone(), (name.clone(), "smpte", Size::HD))
            .unwrap();
    }
    mixer.play();
    mixer.generate_dot_file("test_visible-1.dot");

    sleep(Duration::from_millis(500));
    mixer.set_title("show 1-4");
    mixer.pause();
    mixer.set_visibles(&names[0..4]).unwrap();
    mixer.layout().unwrap();
    mixer.play();

    mixer.generate_dot_file("test_visible-2.dot");

    sleep(Duration::from_millis(500));

    mixer.set_title("show 5-6");
    mixer.pause();
    mixer.set_visibles(&names[4..6]).unwrap();
    mixer.layout().unwrap();
    mixer.play();
    mixer.generate_dot_file("test_visible-3.dot");

    mixer.set_title("show 6-8");
    mixer.pause();
    mixer.set_visibles(&names[5..8]).unwrap();
    mixer.layout().unwrap();
    mixer.play();
    mixer.generate_dot_file("test_visible-4.dot");

    mixer.set_title("show 8");
    mixer.pause();
    mixer.set_visibles(&names[7..8]).unwrap();
    mixer.layout().unwrap();
    mixer.play();
    mixer.generate_dot_file("test_visible-5.dot");

    sleep(Duration::from_millis(500));
}

#[test]
fn test_remove() {
    // initialize gstreamer
    gst::init().unwrap();

    // get output resolution from arguments
    let resolution = Size {
        width: 640,
        height: 480,
    };
    let mut mixer = Mixer::<Grid, TestSource>::new::<DisplaySink>(resolution, 8, 4).unwrap();
    mixer.play();

    mixer.generate_dot_file("test_visible-0.dot");

    let names = names(8);

    sleep(Duration::from_millis(500));

    mixer.set_title("add 8 participants");
    mixer.pause();
    for name in &names {
        mixer
            .add_participant(name.clone(), (name.clone(), "smpte", Size::HD))
            .unwrap();
    }
    mixer.play();
    mixer.generate_dot_file("test_visible-1.dot");

    sleep(Duration::from_millis(500));

    mixer.set_title("show 1-4");
    mixer.pause();
    mixer.set_visibles(&names[0..4]).unwrap();
    mixer.layout().unwrap();
    mixer.play();

    mixer.generate_dot_file("test_visible-2.dot");

    sleep(Duration::from_secs(1));

    mixer.set_title("remove 1-2");
    mixer.pause();
    for name in &names[0..2] {
        mixer.remove_participant(name).unwrap();
    }
    mixer.layout().unwrap();
    mixer.play();
    mixer.generate_dot_file("test_visible-3.dot");

    sleep(Duration::from_secs(1));

    mixer.set_title("show 2-6");
    mixer.pause();
    for name in &names[4..8] {
        mixer.remove_participant(name).unwrap();
    }
    mixer.layout().unwrap();
    mixer.play();
    mixer.generate_dot_file("test_visible-4.dot");

    sleep(Duration::from_secs(1));
}
