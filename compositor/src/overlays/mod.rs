// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

//! Overlays module.
mod clock_overlay;
mod padding_overlay;
mod talk_overlay;
mod text_overlay;

pub use clock_overlay::*;
pub use padding_overlay::*;
pub use talk_overlay::*;
pub use text_overlay::*;

use gst_base::prelude::ElementExt;

/// Trait of overlays as the mixer sees it.
pub trait Overlay: std::fmt::Debug {
    /// Add overlay element
    fn element(&self) -> &gst::Element;
    fn sink(&self) -> Option<gst::Pad> {
        self.element().static_pad("video_sink")
    }
    fn src(&self) -> Option<gst::Pad> {
        self.element().static_pad("src")
    }
    /// show or hide overlay element
    fn show(&self, show: bool);
}
