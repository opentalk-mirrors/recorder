use gst::prelude::ElementExtManual;
use gst_base::prelude::{GstBinExt, PadExt};

use crate::*;

/// Parameters for a multi sink
///
/// Add as many sinks you want and access those through your boxed copy
#[derive(Debug, Default)]
pub struct MultiParameters {
    /// list of output sinks
    pub sinks: Vec<Box<dyn Sink>>,
}

impl MultiParameters {
    pub fn new(sinks: Vec<Box<dyn Sink>>) -> Self {
        Self { sinks }
    }
}

/// This sink holds multiple different sinks and distributes the input
/// it gets from the ghost pads`video` and `audio` to them.
#[derive(Debug)]
pub struct MultiSink {
    /// for later use to access complex sinks (like blinders)
    sinks: Vec<Box<dyn Sink>>,
    /// bin of the multi sink
    bin: gst::Bin,
    /// ghost pad for incoming video
    video: gst::GhostPad,
    /// ghost pad for incoming audio
    audio: gst::GhostPad,
}

impl From<MultiParameters> for MultiSink {
    fn from(params: MultiParameters) -> Self {
        Self::new(params)
    }
}

impl MultiSink {
    /// Create new sink with given parameters
    pub fn new(params: MultiParameters) -> Self {
        trace!("new()");

        // create new GStreamer pipeline
        let bin = gst::parse_bin_from_description(
            r#" 
            name="Play Out Sink"
    
            tee
                name=video

            tee
                name=audio
            "#,
            false,
        )
        .expect("could not parse display link pipeline");

        // link tee sinks to ghostpads (must stay here - before the following code)
        let video = add_ghost_pad(&bin, "video", "sink");
        let audio = add_ghost_pad(&bin, "audio", "sink");

        // get tees from bin
        let video_tee = bin.by_name("video").expect("can't find video tee");
        let audio_tee = bin.by_name("audio").expect("can't find audio tee");

        // connect tees with there channel sinks
        for sink in &params.sinks {
            // add sink to bin
            bin.add(&sink.bin())
                .expect("cannot add sink to play out bin");

            // request new tee src pads for audio and video
            let video_src = video_tee
                .request_pad_simple("src_%u")
                .expect("could not get video src pad at tee");
            let audio_src = audio_tee
                .request_pad_simple("src_%u")
                .expect("could not get audio src pad at tee");

            // link new tee src pads to sink's pads
            video_src
                .link(&sink.video())
                .expect("could not link video tee to sink");
            audio_src
                .link(&sink.audio())
                .expect("could not link audio tee to sink");
        }

        // return new display sink
        MultiSink {
            sinks: params.sinks,
            video,
            audio,
            bin,
        }
    }
}

impl Sink for MultiSink {
    fn video(&self) -> gst::GhostPad {
        self.video.clone()
    }

    fn audio(&self) -> gst::GhostPad {
        self.audio.clone()
    }

    fn bin(&self) -> gst::Bin {
        self.bin.clone()
    }

    fn on_play(&mut self) {
        self.sinks.iter_mut().for_each(|sink| sink.on_play());
    }

    fn on_pause(&mut self) {
        self.sinks.iter_mut().for_each(|sink| sink.on_pause());
    }

    fn on_exit(&mut self, pipeline: &gst::Pipeline) {
        self.sinks
            .iter_mut()
            .for_each(|sink| sink.on_exit(pipeline));
    }
}
