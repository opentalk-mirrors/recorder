const TEST_OUTPUT_DIR: &str = "./test_output";

mod dash;
mod matroska;
mod mixer;
mod mp4;
mod overlays;
mod speaker_mode;
mod stream_status;

use core::{fmt::Debug, hash::Hash, time::Duration};

use crate::*;

pub fn test_name_format() -> TextFormat {
    TextFormat {
        font: Font {
            name: "Sans",
            size: 9,
        },
        align: Align {
            horizontal: HAlign::Left,
            vertical: VAlign::Bottom,
        },
        ..Default::default()
    }
}

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

/// Fake sink to catch the compositor output without any further processing.
pub enum TestSink {
    Fake(FakeSink),
    Display(DisplaySink),
}

fn has_display() -> bool {
    std::env::var("DISPLAY").is_ok()
}

impl Sink for TestSink {
    /// Needs no parameters.
    type Parameters = ();

    /// Create and add new fake sink into existing pipeline.
    fn new(pipeline: &gst::Pipeline, _: ()) -> Self {
        if has_display() {
            debug!("using display sink because display is available");
            Self::Display(DisplaySink::new(pipeline, ()))
        } else {
            debug!("using fake sink");
            Self::Fake(FakeSink::new(pipeline, ()))
        }
    }

    /// Get video sink pad.
    fn video_sink_pad(&self) -> gst::Pad {
        match self {
            Self::Fake(sink) => sink.video_sink_pad(),
            Self::Display(sink) => sink.video_sink_pad(),
        }
    }

    /// Get audio sink pad.
    fn audio_sink_pad(&self) -> gst::Pad {
        match self {
            Self::Fake(sink) => sink.audio_sink_pad(),
            Self::Display(sink) => sink.audio_sink_pad(),
        }
    }
}
