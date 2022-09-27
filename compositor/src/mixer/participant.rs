use crate::error::Error;
use crate::layout::*;
use crate::mixer::Mixer;
use gst::{
    prelude::{ElementExtManual, GObjectExtManualGst},
    traits::{ElementExt, GstBinExt, PadExt},
    PadExtManual, Pipeline,
};
use gstreamer as gst;

#[derive(Debug)]
pub struct Participant {
    pub name: String,
    elements: Vec<gst::Element>,
    pub video_fake_sink: Option<gst::Element>,
    pub video_src_pad: gst::Pad,
    pub video_sink_pad: gst::Pad,
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
}
