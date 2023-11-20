// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

//! Overlay displaying the current time.

use anyhow::{Context, Result};
use gst::prelude::*;

use crate::{Overlay, TextStyle};

/// Overlay displaying current time.
#[derive(Debug, Clone)]
pub struct ClockOverlay {
    // clockoverlay element
    element: gst::Element,
}

impl ClockOverlay {
    /// Create new clock overlay.
    ///
    /// # Arguments
    ///
    /// - `name`: Element's name.
    /// - `format`: Clock format string.
    /// - `style`: Style of the clock display.
    ///
    /// # Errors
    ///
    /// This can fail if the `clockoverlay` cannot be created in `GStreamer`.
    pub fn create(name: &str, format: &str, style: TextStyle) -> Result<Self> {
        trace!("new( {format:?}, {style:?} )");

        // create text overlay
        let element = gst::ElementFactory::make_with_name("clockoverlay", Some(name))
            .context("failed to create clock overlay")?;

        // set up properties
        element.set_property("time-format", format);
        element.set_property(
            "font-desc",
            format!(
                "{name},{size}",
                name = style.font.name,
                size = style.font.size
            ),
        );
        element.set_property("xpad", style.padding.x);
        element.set_property("ypad", style.padding.y);
        element.set_property("color", style.color);
        element.set_property_from_str("halignment", style.align.horizontal.into());
        element.set_property_from_str("valignment", style.align.vertical.into());
        element.set_property("auto-resize", false);

        // return Overlay
        Ok(Self { element })
    }
}

impl Overlay for ClockOverlay {
    #[must_use]
    fn element(&self) -> &gst::Element {
        &self.element
    }
    fn show(&self, show: bool) {
        self.element.set_property("silent", !show);
    }
}
