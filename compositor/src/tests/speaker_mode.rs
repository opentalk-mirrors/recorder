use crate::*;
use core::time::Duration;
use std::thread::sleep;

#[test]
fn test_speaker_mode() {
    // init logger
    let _ = env_logger::try_init();
    // initialize gstreamer
    gst::init().unwrap();

    // get output resolution from arguments
    let resolution = Size {
        width: 640,
        height: 480,
    };
    let mut mixer = Mixer::<Speaker, TestSource, DisplaySink, u32>::new(
        resolution,
        4,
        6,
        (),
        SpeakerMode::FirstShift,
    )
    .unwrap();
    mixer.play();

    mixer.generate_dot_file("test_speaker_mode-0", gst::DebugGraphDetails::ALL);

    let participants = super::generate_ids(5);
    let ids: Vec<u32> = participants.iter().map(|p| p.0).collect();

    sleep(Duration::from_millis(500));

    mixer.set_title("add 8 participants");
    mixer.pause();
    let resolutions = [Size::SD, Size::HD, Size::FHD, Size::QHD, Size::UHD];
    let images = [
        "images/participant_SD.png",
        "images/participant_HD.png",
        "images/participant_FHD.png",
        "images/participant_QHD.png",
        "images/participant_UHD.png",
    ];
    for (i, (id, name)) in participants.iter().enumerate() {
        let params = TestSourceParameters {
            resolution: resolutions[i],
            pattern: Pattern::Location(images[i].into()),
        };
        mixer.add_participant(*id, name.clone(), params).unwrap();
    }
    mixer.set_visibles(&ids[0..4]).unwrap();

    mixer.play();

    sleep(Duration::from_millis(500));

    for i in 0..5 {
        mixer.set_title(&format!("Speaker {i}"));
        mixer.pause();
        mixer.set_speaker(Some(i)).unwrap();
        mixer.play();

        mixer.generate_dot_file(
            &format!("test_speaker_mode-{}", i + 1),
            gst::DebugGraphDetails::ALL,
        );

        sleep(Duration::from_millis(500));
    }
}
