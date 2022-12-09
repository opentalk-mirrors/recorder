const TEST_OUTPUT_DIR: &str = "./test_output";

mod dash;
mod matroska;
mod mixer;
mod mp4;
mod speaker_mode;

use core::{fmt::Debug, hash::Hash, time::Duration};

use crate::*;

fn generate_ids<ID>(count: u32) -> Vec<(ID, String)>
where
    ID: Eq + Ord + Hash + Copy + Debug + From<u32>,
{
    // generate participant IDs and names
    (0..count)
        .map(|n| (n.into(), format!("Participant {n:?}")))
        .collect()
}

fn generate_participants<L, SINK, ID>(
    mixer: &mut Mixer<L, TestSource, SINK, ID>,
    n: u32,
) -> (Vec<(ID, String)>, Vec<ID>)
where
    L: Layout,
    SINK: crate::Sink,
    ID: Eq + Ord + Hash + Copy + Debug + From<u32>,
{
    let participants = generate_ids(n);
    let ids: Vec<ID> = participants.iter().map(|p| p.0).collect();

    mixer.set_title(&format!("add {n} participants"));
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
            resolution: resolutions[i % images.len()],
            pattern: Pattern::Location(images[i % images.len()].into()),
            name: Some(name.clone()),
        };
        mixer.add_participant(*id, name.clone(), params).unwrap();
    }
    (participants, ids)
}

fn wait_secs(sec: u64) {
    debug!("waiting {sec} second(s)...");
    std::thread::sleep(Duration::from_secs(sec));
    debug!("...waited {sec} second(s).");
}

fn wait_millis(milli_sec: u64) {
    debug!("waiting {milli_sec} millisecond(s)...");
    std::thread::sleep(Duration::from_millis(milli_sec));
    debug!("...waited {milli_sec} millisecond(s).");
}
