// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

//! Sink trait.

use std::fmt::Debug;

use anyhow::Result;
use glib::Cast;
use gst::{
    prelude::*, ClockTime, Element, ElementFactory, Fraction, GhostPad, MessageType, Pipeline,
};
use gst_app::AppSrc;
use gst_base::prelude::ElementExt;

use super::{audio_mixer::AudioMixer, video_mixer::VideoMixer};
use crate::{
    debug, GstBinErrorExt, GstElementBuilderErrorExt, GstElementErrorExt, GstGhostPadErrorExt,
    GstPadErrorExt, AUDIO_CHANNELS, AUDIO_SAMPLE_RATE, VIDEO_FRAMERATE, VIDEO_HEIGHT, VIDEO_WIDTH,
};

/// Trait of an output sink.
pub trait Sink: Send + Debug + 'static {
    /// Get sink pad of the video sink.
    fn video(&self) -> Option<gst::GhostPad>;

    /// Get sink pad of the audio sink.
    fn audio(&self) -> gst::GhostPad;

    fn bin(&self) -> gst::Bin;

    /// Called by `Mixer::play()`.
    ///
    /// # Errors
    ///
    /// This cannot fail, it's doing nothing.
    fn on_play(&mut self) -> Result<()> {
        Ok(())
    }

    /// Called by `Mixer::pause()`.
    fn on_pause(&mut self) {}

    /// Does the sink pipeline require an eos signal before nulling
    fn requires_eos(&self) -> bool {
        true
    }
}

#[derive(Debug)]
pub(crate) struct ActiveSink {
    pub(crate) pipeline: Pipeline,
    // The sink needs to be hold until it's dropped at the end
    pub(crate) inner: Box<dyn Sink>,
}

impl ActiveSink {
    /// Link the given sink to the `audio_mixer`.
    ///
    /// # Errors
    ///
    /// This can fail if the audio sink could not be linked to the `audio_mixer`.
    pub(crate) fn link_audio_mixer(&self, audio_mixer: &AudioMixer) -> Result<()> {
        let app_src = AppSrc::builder()
            .name("audiosrc")
            .caps(
                &gst::Caps::builder("audio/x-raw")
                    .field("format", "S16LE")
                    .field("layout", "interleaved")
                    .field("rate", AUDIO_SAMPLE_RATE)
                    .field("channels", AUDIO_CHANNELS)
                    .build(),
            )
            .min_latency(200_000_000i64)
            .format(gst::Format::Time)
            .max_bytes(1)
            .block(true)
            .is_live(true)
            .build();
        let queue = ElementFactory::make("queue")
            .property_from_str("leaky", "downstream")
            .build_with_context()?;
        let audioconvert = ElementFactory::make("audioconvert").build_with_context()?;

        self.pipeline
            .add_many_with_context(&[app_src.upcast_ref(), &queue, &audioconvert])?;

        Element::link_many_with_context(&[app_src.upcast_ref(), &queue, &audioconvert])?;

        audioconvert
            .static_pad_with_context("src")?
            .link_with_context(&self.inner.audio())?;

        audio_mixer.link_sink(&app_src);

        Ok(())
    }

    /// Link the given sink to the `video_mixer`.
    ///
    /// # Errors
    ///
    /// This can fail if the video sink could not be linked to the `video_mixer`.
    pub(crate) fn link_video_mixer(&self, video_mixer: &VideoMixer) -> Result<()> {
        let Some(video_sink) = &self.inner.video() else {
            return Ok(());
        };

        let app_src = AppSrc::builder()
            .name("videosrc")
            .caps(
                &gst::Caps::builder("video/x-raw")
                    .field("format", "I420")
                    .field("width", VIDEO_WIDTH)
                    .field("height", VIDEO_HEIGHT)
                    .field("framerate", Fraction::new(VIDEO_FRAMERATE, 1))
                    .build(),
            )
            .min_latency(200_000_000i64)
            .format(gst::Format::Time)
            .max_bytes(1)
            .block(true)
            .is_live(true)
            .build();
        let queue = ElementFactory::make("queue")
            .property_from_str("leaky", "downstream")
            .build_with_context()?;
        let videoconvert = ElementFactory::make("videoconvert").build_with_context()?;

        self.pipeline
            .add_many_with_context(&[app_src.upcast_ref(), &queue, &videoconvert])?;

        Element::link_many_with_context(&[app_src.upcast_ref(), &queue, &videoconvert])?;

        videoconvert
            .static_pad_with_context("src")?
            .link_with_context(video_sink)?;

        video_mixer.link_sink(&app_src);

        Ok(())
    }
}

impl Drop for ActiveSink {
    fn drop(&mut self) {
        debug!("Dropping ActiveSink...");
        debug::debug_dot(&self.pipeline, "SINK-DROP");

        if self.inner.requires_eos() {
            debug!("Send EOS to pipeline");

            self.pipeline.send_event(gst::event::Eos::new());

            debug!("Wait for EOS to be done");

            if let Some(bus) = self.pipeline.bus() {
                if bus
                    .timed_pop_filtered(ClockTime::NONE, &[MessageType::Eos])
                    .is_none()
                {
                    error!("unable to send the EOS");
                }
            } else {
                error!("Unable to send EOS, there is no bus in the pipeline");
            }
        }

        debug!("Nulling Pipeline...");
        if let Err(error) = self.pipeline.set_state_with_context(gst::State::Null) {
            error!("Unable to set the pipeline to the `Null` state, error: {error}");
        }

        debug!("Nulling Pipeline completed, remove sink");
    }
}

/// Adds a `GhostPad` to the given `Bin`.
///
/// # Errors
///
/// There are three reasons why this could fail:
/// - The element name cannot be found in the bin.
/// - The pad cannot be found in the element.
/// - The `GhostPad` cannot be added to the bin.
#[allow(clippy::must_use_candidate)]
pub fn add_ghost_pad(bin: &gst::Bin, name: &str, pad: &str) -> Result<gst::GhostPad> {
    trace!(
        "add_ghost_pad({bin}, {name}, {pad}) ",
        bin = debug::name(bin)
    );
    let pad = bin
        .by_name_with_context(name)?
        .static_pad_with_context(pad)?;
    let ghost_pad = GhostPad::with_target_with_context(Some(name), &pad)?;
    bin.add_pad_with_context(&ghost_pad)?;

    Ok(ghost_pad)
}
