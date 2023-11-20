// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

/*!
    # Purpose
    The *compositor* crate manages a [GStreamer](https://gstreamer.freedesktop.org/) pipeline which receives [WebRTC](https://webrtc.org/) input audio and video streams
    of so-called *streams* and mixes them together using the so-called *mixer*. While *talk* manages multiple stream participants and visibility.

    - [Talk]
    - [Mixer]
    - [Stream]

    It then composes an output image showing some of them (so-called *visibles*) in the output picture.

    All incoming audio of all the streams will be mixed together independent of if they are invisible or not.
    The output then will be written onto disk into a
    [MPEG-DASH](https://de.wikipedia.org/wiki/Dynamic_Adaptive_Streaming_over_HTTP (Dynamic Adaptive Streaming over HTTP))
    instance which consists of several files including the
    [MPD](https://ott.dolby.com/OnDelKits/DDP/Dolby_Digital_Plus_Online_Delivery_Kit_v1.5/Documentation/Playback/SDM/help_files/topics/c_dash_mpd_ov.html (media presentation description))
    and Transport Streams.

    # Source & Sink

    To read the input and write the output the following types are used

    - [WebRtcSource]
      manages a connection to a WebRTC source and provides the content to the internal GStreamer pipeline.
    - [AnySink]
      is an universal sink that can be one of the following.
    - [DashSink]
      writes the output into a Dash instance consisting of an MPD file and several audio/video files.
    - [Mp4Sink]
      writes the output into a MPEG4 file.
    - [MatroskaSink]
      listens on a TCP port to write the raw output to, after someone connects.
    - [TestBlinder]
      blinds it's input with an alternative input (currently test source)
    - [MultiSink]
      distributes one output to multiple different output sinks.

    # Layouts

    Several so-called *layouts* can be used to control the output composite.

    - [Grid]
      shows a grid of all visible streams
    - [Speaker]
      shows a bigger picture of the first visible stream (so-called *speaker*)
      and uses the rest of the available picture area to arrange all other visibles.

    # Overlays

    A *talk* uses overlays to display titles, clock, etc. which can be configured.

    - [AnyOverlay]
      A generic overlay type which can contain any other overlay.
    - [TextOverlay]
      Overlay which displays a changeable text.
    - [ClockOverlay]
      Overlay which displays current time.
    - [TalkOverlay]
      Combined Text and Clock Overlay which is used in Talk.

    # Generic traits for extending capabilities

    - [Source]
      is a trait which the mixer is assuming for an input source.
    - [Sink]
      is a trait which the mixer is assuming for an output sink.
    - [Layout]
      is a trait which the mixer is assuming for display layout of the recording
    - [Overlay]
      is a trait for overlays.
    - [Blinder]
      gives access to a blinder sink.

    # Testing

    In addition there are some alternative sources and sinks included which are used for testing purposes.

    - [TestSource]
      which just generates some dummy stream audio and video data.
    - [FakeSink]
      is a sink without any output - just to make it run.
    - [DisplaySink]
      is a sink which displays the output on the screen.

    # Debugging

    Some debug tools:

    - [dot](debug::dot),  [debug_dot](debug::debug_dot) and [dot_ext](debug::dot_ext)
      Makes the debug DOT feature of *gstreamer* more convenient for big number of output files
    - [name](debug::name)
      Generates a name from an element which includes parent names for better tracing.
*/

#![allow(clippy::module_name_repetitions)]

#[macro_use]
extern crate log;

pub mod layout;
mod mixer;
mod overlays;
mod sinks;
mod sources;

#[cfg(test)]
mod tests;

pub use layout::*;
pub use mixer::*;
pub use overlays::*;
pub use sinks::*;
pub use sources::*;

#[cfg(test)]
pub use tests::testing;
