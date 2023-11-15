// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

//! Overlays.

use gst_base::prelude::ElementExt;

use crate::{ClockOverlay, TalkOverlay, TextOverlay};

/// Trait of overlays as the mixer sees it.
pub trait Overlay {
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

/// enum which bundles several types of overlays
#[derive(Debug, Clone)]
pub enum AnyOverlay {
    /// Text overlay
    Text(TextOverlay),
    /// Clock overlay
    Clock(ClockOverlay),
    /// Participant overlay
    Talk(TalkOverlay),
}

impl Overlay for AnyOverlay {
    fn element(&self) -> &gst::Element {
        match self {
            Self::Text(o) => o.element(),
            Self::Clock(o) => o.element(),
            Self::Talk(o) => o.element(),
        }
    }
    fn show(&self, show: bool) {
        match self {
            Self::Text(o) => o.show(show),
            Self::Clock(o) => o.show(show),
            Self::Talk(o) => o.show(show),
        }
    }
}

impl From<TextOverlay> for AnyOverlay {
    fn from(overlay: TextOverlay) -> AnyOverlay {
        AnyOverlay::Text(overlay)
    }
}

impl From<ClockOverlay> for AnyOverlay {
    fn from(overlay: ClockOverlay) -> AnyOverlay {
        AnyOverlay::Clock(overlay)
    }
}

impl From<TalkOverlay> for AnyOverlay {
    fn from(overlay: TalkOverlay) -> AnyOverlay {
        AnyOverlay::Talk(overlay)
    }
}
