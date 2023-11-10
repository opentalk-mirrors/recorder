// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use gst::prelude::{ElementExtManual, GstBinExtManual};
use gst_base::prelude::{ElementExt, GstBinExt, PadExt};

use crate::{add_ghost_pad, Sink};

/// Parameters for a multi sink
///
/// Add as many sinks you want and access those through your boxed copy
#[derive(Debug, Default)]
pub struct MultiParameters {
    /// list of output sinks
    pub sinks: Vec<Box<dyn Sink>>,
}

impl MultiParameters {
    #[must_use]
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
    ///
    /// # Panics
    ///
    /// This can panic if the 'tee' bin can't be created.
    #[must_use]
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
            // create a queue for each channel's tee
            let video_queue = gst::ElementFactory::make_with_name("queue", None).unwrap();
            let audio_queue = gst::ElementFactory::make_with_name("queue", None).unwrap();

            // add sink and queues to bin
            bin.add_many(&[&video_queue, &audio_queue, sink.bin().as_ref()])
                .expect("cannot add elements to multi sink bin");

            // link tees to queues
            video_tee.link(&video_queue).unwrap();
            audio_tee.link(&audio_queue).unwrap();

            // link new tee src pads to sink's pads
            video_queue
                .static_pad("src")
                .expect("cant find src of video queue")
                .link(&sink.video())
                .expect("could not link video tee to sink");
            audio_queue
                .static_pad("src")
                .expect("cant find src of audio queue")
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
    #[must_use]
    fn video(&self) -> gst::GhostPad {
        self.video.clone()
    }

    #[must_use]
    fn audio(&self) -> gst::GhostPad {
        self.audio.clone()
    }

    #[must_use]
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
