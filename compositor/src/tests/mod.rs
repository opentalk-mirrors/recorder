const TEST_OUTPUT_DIR: &str = "./test_output";

mod dash;
mod matroska;
mod mixer;
mod mp4;
mod speaker_mode;

use core::{fmt::Debug, hash::Hash};

pub fn generate_ids<ID>(count: u32) -> Vec<(ID, String)>
where
    ID: Eq + Ord + Hash + Copy + Debug + From<u32>,
{
    // add participant names
    (0..count)
        .map(|n| (n.into(), format!("Participant {n:?}")))
        .collect()
}

fn add_participants<L, SINK, ID>(
    mixer: &mut crate::Mixer<L, crate::TestSource, SINK, ID>,
    n: u32,
) -> (Vec<(ID, String)>, Vec<ID>)
where
    L: crate::Layout,
    SINK: crate::Sink,
    ID: Eq + Ord + Hash + Copy + Debug + From<u32>,
{
    use crate::{Pattern, Size, TestSourceParameters};

    let participants = generate_ids(n);
    let ids: Vec<ID> = participants.iter().map(|p| p.0).collect();

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
            pattern: Pattern::Location(images[i % images.len()].into()),
        };
        mixer.add_participant(*id, name.clone(), params).unwrap();
    }
    (participants, ids)
}
