const TEST_OUTPUT_DIR: &str = "./test_output";

mod dash;
mod matroska;
mod mixer;
mod mp4;
mod speaker_mode;

pub fn generate_ids(count: u32) -> Vec<(u32, String)> {
    // add participant names
    (0..count)
        .map(|n| (n, format!("Participant {n}")))
        .collect()
}
