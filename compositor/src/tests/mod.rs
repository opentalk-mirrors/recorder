const TEST_OUTPUT_DIR: &str = "./test_output";

mod dash;
mod matroska;
mod mixer;
mod mp4;
mod speaker_mode;
mod stream_status;

use core::{fmt::Debug, hash::Hash, time::Duration};

use crate::*;

fn generate_ids<ID>(count: u32) -> Vec<(ID, String)>
where
    ID: Eq + Ord + Hash + Copy + Debug + From<u32>,
{
    // generate stream IDs and names
    (0..count)
        .map(|n| (n.into(), format!("Participant {n:?}")))
        .collect()
}

fn generate_streams<L, SINK, ID>(
    mixer: &mut Mixer<L, TestSource, SINK, ID>,
    n: u32,
) -> (Vec<(ID, String)>, Vec<ID>)
where
    L: Layout,
    SINK: crate::Sink,
    ID: Eq + Ord + Hash + Copy + Debug + From<u32>,
{
    let streams = generate_ids(n);
    let ids: Vec<ID> = streams.iter().map(|p| p.0).collect();

    mixer.set_title(&format!("add {n} streams"));
    let resolutions = [Size::SD, Size::HD, Size::FHD, Size::QHD, Size::UHD];
    let images = [
        "images/participant_SD.png",
        "images/participant_HD.png",
        "images/participant_FHD.png",
        "images/participant_QHD.png",
        "images/participant_UHD.png",
    ];
    for (i, (id, name)) in streams.iter().enumerate() {
        let params = TestSourceParameters {
            resolution: resolutions[i % images.len()],
            pattern: Pattern::Location(images[i % images.len()].into()),
            name: Some(name.clone()),
        };
        mixer.add_stream(*id, name.clone(), params).unwrap();
    }
    (streams, ids)
}

fn wait_secs(sec: u64) {
    debug!("waiting {sec} second(s)...");
    std::thread::sleep(Duration::from_secs(sec));
    debug!("...waited {sec} second(s).");
}

fn wait_millis(milliseconds: u64) {
    debug!("waiting {milliseconds} millisecond(s)...");
    std::thread::sleep(Duration::from_millis(milliseconds));
    debug!("...waited {milliseconds} millisecond(s).");
}
