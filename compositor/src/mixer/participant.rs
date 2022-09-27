use crate::layout::*;
use crate::mixer::Mixer;
use gst::{
    prelude::{ElementExtManual, GObjectExtManualGst},
    traits::{ElementExt, GstBinExt, PadExt},
    PadExtManual, Pipeline,
};
use gstreamer as gst;

#[derive(Debug)]
pub enum Error {
    AlreadyLinked,
    AlreadyUnlinked,
}

#[derive(Debug)]
pub struct Participant {
    name: String,
    pipeline: Pipeline,
    elements: Vec<gst::Element>,
    video_fake_sink: Option<gst::Element>,
    video_src_pad: gst::Pad,
    video_sink_pad: gst::Pad,
    /*    audio_src_pad: gst::Pad,
    audio_fakesink_pad: gst::Pad,
     */
}

impl Participant {
    #[allow(dead_code)]
    pub fn create(
        pipeline: &gst::Pipeline,
        name: &str,
        pattern: &str,
        resolution: &Size,
    ) -> Participant {
        let width = resolution.width;
        let height = resolution.height;

        // create test src
        let video_test_src =
            gst::ElementFactory::make("videotestsrc", Some(&format!("video-testsrc-{name}")))
                .unwrap();
        video_test_src.set_property_from_str("pattern", pattern);
        video_test_src.set_property_from_str("is-live", "true");

        // create caps setter
        let video_caps =
            gst::ElementFactory::make("capssetter", Some(&format!("video-caps-{name}"))).unwrap();
        video_caps.set_property_from_str(
            "caps",
            &format!("video/x-raw,format=RGB,width={width},height={height}",),
        );

        let video_queue =
            gst::ElementFactory::make("queue", Some(&format!("video-src-{name}"))).unwrap();

        // create fake sink
        let video_fake_sink =
            gst::ElementFactory::make("fakesink", Some(&format!("fakesink-{name}"))).unwrap();
        video_fake_sink.set_property_from_str("sync", "true");

        /*
               // create test src
               let audio_test_src =
                   gst::ElementFactory::make("audiotestsrc", Some(&format!("audio-testsrc-{name}")))
                       .unwrap();
               audio_test_src.set_property_from_str("volume", "0.01");

               // create caps setter
               let audio_caps =
                   gst::ElementFactory::make("capssetter", Some(&format!("audio-src-{name}"))).unwrap();
               video_caps.set_property_from_str(
                   "caps",
                   &format!("audio/x-raw,format=S16LW,channels=2,layout=interleaved,rate=48000",),
               );

               // create fake sink
               let audio_fake_sink =
                   gst::ElementFactory::make("fakesink", Some(&format!("audio-fakesink-{name}"))).unwrap();
               audio_fake_sink.set_property_from_str("sync", "true");
        */
        // add elements to pipeline
        pipeline.add(&video_test_src).unwrap();
        pipeline.add(&video_caps).unwrap();
        pipeline.add(&video_queue).unwrap();
        pipeline.add(&video_fake_sink).unwrap();
        /*        pipeline.add(&audio_test_src).unwrap();
               pipeline.add(&audio_fake_sink).unwrap();
        */
        // link elements
        video_test_src.link(&video_caps).unwrap();
        video_caps.link(&video_queue).unwrap();
        video_queue.link(&video_fake_sink).unwrap();
        //audio_test_src.link(&audio_fake_sink).unwrap();

        Participant {
            name: name.to_string(),
            pipeline: pipeline.clone(),
            // remember elements for deletion
            elements: vec![
                video_test_src.clone(),
                video_caps.clone(),
                video_fake_sink.clone(),
                /*                 audio_test_src.clone(),
                             audio_fake_sink.clone(),
                */
            ],
            // remember elements and pads for connect/disconnect
            video_src_pad: video_queue.static_pad("src").unwrap(),
            video_sink_pad: video_fake_sink.static_pad("sink").unwrap(),
            video_fake_sink: Some(video_fake_sink),
            /*         audio_src_pad: audio_test_src.static_pad("src").unwrap(),
            audio_fakesink_pad: audio_fake_sink.static_pad("sink").unwrap(),
             */
        }
    }
    pub fn link<L>(&mut self, mixer: &Mixer<L>) -> Result<(), Error>
    where
        L: Layout,
    {
        trace!("linking {name}...", name = self.name);
        // check if not already linked to compositor
        if let Some(fake_sink) = &self.video_fake_sink {
            // sync closure with channel
            let (notify, wait) = std::sync::mpsc::sync_channel(1);
            // add probe to stop source
            self.video_src_pad.add_probe(gst::PadProbeType::BLOCK, {
                // clone elements and pads for closure
                let sink_pad = self.video_sink_pad.clone();
                let src_pad = self.video_src_pad.clone();
                let fake_sink = fake_sink.clone();
                let compositor = mixer.compositor.clone();
                let pipeline = self.pipeline.clone();
                move |_pad, info| {
                    src_pad.remove_probe(info.id.take().unwrap());
                    // we only want the first probe event
                    if src_pad.unlink(&sink_pad).is_ok() {
                        trace!("unlinking video fake sink pad...");
                        // halt fake sink
                        fake_sink.set_state(gst::State::Null).unwrap();
                        // remove fake sink from pipeline
                        pipeline.remove(&fake_sink).unwrap();
                        // create new compositor sink pad
                        let compositor_sink_pad = compositor.request_pad_simple("sink_%u").unwrap();
                        // link source with compositor
                        src_pad.link(&compositor_sink_pad).unwrap();
                        // sync with outside and send compositor sink pad
                        notify.send(compositor_sink_pad).unwrap();
                    }
                    // we did already remove the probe
                    gst::PadProbeReturn::Handled
                }
            });

            // wait for closure to finish and retrieve compositor sink pad
            let compositor_sink_pad = wait
                .recv_timeout(std::time::Duration::from_secs(6))
                .unwrap();

            // remove fake sink from compositor to signal that we have unlinked it
            self.video_fake_sink = None;
            // save new compositor sink pad
            self.video_sink_pad = compositor_sink_pad;
            trace!("linked {name} successfully", name = self.name);
            mixer.layout();
            // done
            Ok(())
        } else {
            Err(Error::AlreadyLinked)
        }
    }
    pub fn unlink<L>(&mut self, mixer: &Mixer<L>) -> Result<(), Error>
    where
        L: Layout,
    {
        trace!("unlinking {name}...", name = self.name);
        // check if not already linked to fake sink
        if self.video_fake_sink.is_none() {
            // sync closure with channel
            let (notify, wait) = std::sync::mpsc::sync_channel(1);
            // create fake sink
            trace!("creating new fake sink...");
            let fake_sink = gst::ElementFactory::make(
                "fakesink",
                Some(&format!("fakesink-{name}", name = self.name)),
            )
            .unwrap();
            fake_sink.set_property_from_str("sync", "true");
            // add probe to stop source
            self.video_src_pad.add_probe(gst::PadProbeType::BLOCK, {
                // clone elements and pads for closure
                let src_pad = self.video_src_pad.clone();
                let fake_sink = fake_sink.clone();
                let mixer = mixer.compositor.clone();
                let pipeline = self.pipeline.clone();
                move |pad, info| {
                    // we only want the first probe event
                    trace!("unlinking compositor...");
                    src_pad.remove_probe(info.id.take().unwrap());
                    if let Some(peer) = pad.peer() {
                        mixer.release_request_pad(&peer);

                        trace!("add fake sink to pipeline...");
                        fake_sink.set_state(gst::State::Playing).unwrap();
                        pipeline.add(&fake_sink).unwrap();
                        trace!("create fake sink pad...");
                        let fake_sink_pad = fake_sink.static_pad("sink").unwrap();
                        trace!("link to fake sink pad...");
                        src_pad.link(&fake_sink_pad).unwrap();
                        notify.send(fake_sink_pad).unwrap();
                    }
                    gst::PadProbeReturn::Handled
                }
            });

            let fake_sink_pad = wait
                .recv_timeout(std::time::Duration::from_secs(6))
                .unwrap();

            self.video_fake_sink = Some(fake_sink);
            self.video_sink_pad = fake_sink_pad;

            trace!("unlinked {name} successfully", name = self.name);
            mixer.layout();
            Ok(())
        } else {
            return Err(Error::AlreadyUnlinked);
        }
    }
}
