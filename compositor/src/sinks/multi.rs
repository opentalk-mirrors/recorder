// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use anyhow::{Context, Result};
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

impl MultiSink {
    /// Create new sink with given parameters
    ///
    /// # Errors
    ///
    /// This can throw an error if the `tee` cannot created for `GStreamer`.
    /// Or if adding the `GhostPad` is failing for the `video` and `audio` sink
    pub fn create(params: MultiParameters) -> Result<Self> {
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
        .context("could not parse display link pipeline")?;

        // link tee sinks to ghostpads (must stay here - before the following code)
        let video = add_ghost_pad(&bin, "video", "sink")
            .context("unable to add GhostPad for video sink")?;
        let audio = add_ghost_pad(&bin, "audio", "sink")
            .context("unable to add GhostPad for audio sink")?;

        // get tees from bin
        let video_tee = bin.by_name("video").context("can't find video tee")?;
        let audio_tee = bin.by_name("audio").context("can't find audio tee")?;

        // connect tees with there channel sinks
        for sink in &params.sinks {
            // create a queue for each channel's tee
            let video_queue = gst::ElementFactory::make_with_name("queue", None)
                .context("unable to create queue for video")?;
            let audio_queue = gst::ElementFactory::make_with_name("queue", None)
                .context("unable to create queue for audio")?;

            // add sink and queues to bin
            bin.add_many(&[&video_queue, &audio_queue, sink.bin().as_ref()])
                .context("cannot add elements to multi sink bin")?;

            // link tees to queues
            video_tee
                .link(&video_queue)
                .context("unable to link video_queue with video_tee")?;
            audio_tee
                .link(&audio_queue)
                .context("unable to link autio_queue with audio_tee")?;

            if let Some(video_sink) = &sink.video() {
                video_queue
                    .static_pad("src")
                    .context("cant find src of video queue")?
                    .link(video_sink)
                    .context("could not link video tee to sink")?;
            } else {
                let fakesink = gst::ElementFactory::make("fakesink").build()?;
                bin.add(&fakesink)
                    .context("unable to add `fakesink` to `bin`")?;
                let fakesink_sink_pad = fakesink
                    .static_pad("sink")
                    .context("unable to get static pad `sink` from `fakesink`")?;
                video_queue
                    .static_pad("src")
                    .context("cant find src of video queue")?
                    .link(&fakesink_sink_pad)
                    .context("could not link video tee to sink")?;
                fakesink
                    .sync_state_with_parent()
                    .context("unable to sync `fakesink` with parent")?;
            }

            // link new tee src pads to sink's pads
            audio_queue
                .static_pad("src")
                .context("cant find src of audio queue")?
                .link(&sink.audio())
                .context("could not link audio tee to sink")?;
        }

        Ok(MultiSink {
            sinks: params.sinks,
            video,
            audio,
            bin,
        })
    }
}

impl Sink for MultiSink {
    #[must_use]
    fn video(&self) -> Option<gst::GhostPad> {
        Some(self.video.clone())
    }

    #[must_use]
    fn audio(&self) -> gst::GhostPad {
        self.audio.clone()
    }

    #[must_use]
    fn bin(&self) -> gst::Bin {
        self.bin.clone()
    }

    fn on_play(&mut self) -> Result<()> {
        self.sinks
            .iter_mut()
            .try_for_each(|sink| sink.on_play())
            .context("unable to call on_exit on every sink")
    }

    fn on_pause(&mut self) {
        self.sinks.iter_mut().for_each(|sink| sink.on_pause());
    }

    fn on_exit(&mut self, pipeline: &gst::Pipeline) -> Result<()> {
        self.sinks
            .iter_mut()
            .try_for_each(|sink| sink.on_exit(pipeline))
            .context("unable to call on_exit on every sink")
    }
}
