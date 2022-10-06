extern crate clap;

#[cfg(test)]
use crate::{layout::*, mixer::*};
#[cfg(test)]
use gstreamer as gst;

#[cfg(test)]
fn names(count: usize) -> Vec<String> {
    // add participant names
    (0..count)
        .enumerate()
        .map(|(n, _)| format!("Participant {n}").clone())
        .collect()
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
    let mut mixer = Mixer::<Grid, TestSource>::new::<DisplaySink>(&resolution, 8, 4).unwrap();
    mixer.play();

    mixer.generate_dot_file(&format!("test_visible-0.dot"));

    let names = names(8);

    std::thread::sleep_ms(500);

    mixer.set_title("add 8 participants");
    mixer.pause();
    mixer.add_participants(&names).unwrap();
    mixer.play();
    mixer.generate_dot_file("test_visible-1.dot");

    std::thread::sleep_ms(500);

    mixer.set_title("show 1-4");
    mixer.pause();
    mixer.set_visibles(&names[0..4]).unwrap();
    mixer.layout().unwrap();
    mixer.play();

    mixer.generate_dot_file(&format!("test_visible-2.dot"));
    std::thread::sleep_ms(1000);

    mixer.set_title("show 5-6");
    mixer.pause();
    mixer.set_visibles(&names[4..6]).unwrap();
    mixer.layout().unwrap();
    mixer.play();
    mixer.generate_dot_file(&format!("test_visible-3.dot"));

    mixer.set_title("show 6-8");
    mixer.pause();
    mixer.set_visibles(&names[5..8]).unwrap();
    mixer.layout().unwrap();
    mixer.play();
    mixer.generate_dot_file(&format!("test_visible-4.dot"));

    mixer.set_title("show 8");
    mixer.pause();
    mixer.set_visibles(&names[7..8]).unwrap();
    mixer.layout().unwrap();
    mixer.play();
    mixer.generate_dot_file(&format!("test_visible-5.dot"));

    std::thread::sleep_ms(500);
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
    let mut mixer = Mixer::<Grid, TestSource>::new::<DisplaySink>(&resolution, 8, 4).unwrap();
    mixer.play();

    mixer.generate_dot_file(&format!("test_visible-0.dot"));

    let names = names(8);

    std::thread::sleep_ms(500);

    mixer.set_title("add 8 participants");
    mixer.pause();
    mixer.add_participants(&names).unwrap();
    mixer.play();
    mixer.generate_dot_file(&format!("test_visible-1.dot"));

    std::thread::sleep_ms(500);

    mixer.set_title("show 1-4");
    mixer.pause();
    mixer.set_visibles(&names[0..4]).unwrap();
    mixer.layout().unwrap();
    mixer.play();

    mixer.generate_dot_file(&format!("test_visible-2.dot"));
    std::thread::sleep_ms(1000);

    mixer.set_title("remove 1-2");
    mixer.pause();
    mixer.remove_participants(&names[0..2]).unwrap();
    mixer.layout().unwrap();
    mixer.play();
    mixer.generate_dot_file(&format!("test_visible-3.dot"));

    std::thread::sleep_ms(1000);

    mixer.set_title("show 2-6");
    mixer.pause();
    mixer.set_visibles(&names[4..8]).unwrap();
    mixer.layout().unwrap();
    mixer.play();
    mixer.generate_dot_file(&format!("test_visible-4.dot"));

    std::thread::sleep_ms(1000);
}
