// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

//! Overlay which adds extra padding to the picture

use anyhow::{Context, Result};
use gst::prelude::*;

use crate::Overlay;

/// Text overlay.
#[derive(Debug, Clone)]
pub struct PaddingOverlay {
    element: gst::Element,
}

#[derive(Default, Debug)]
pub struct Padding {
    pub left: i32,
    pub right: i32,
    pub top: i32,
    pub bottom: i32,
}

impl PaddingOverlay {
    /// Creates a new padding overlay.
    ///
    /// # Errors
    ///
    /// This can fail if the `videobox` cannot be created in `GStreamer`.
    pub fn create(name: &str, padding: &Padding) -> Result<Self> {
        trace!("new( {padding:?} )");

        // create videobox element
        let element = gst::ElementFactory::make_with_name("videobox", Some(name))
            .context("failed to create videobox element")?;

        // set up properties
        element.set_property("left", -padding.left);
        element.set_property("right", -padding.right);
        element.set_property("top", -padding.top);
        element.set_property("bottom", -padding.bottom);

        // return Overlay
        Ok(Self { element })
    }
}

impl Overlay for PaddingOverlay {
    #[must_use]
    fn element(&self) -> &gst::Element {
        &self.element
    }

    fn show(&self, _: bool) {
        unimplemented!()
    }

    #[must_use]
    fn sink(&self) -> Option<gst::Pad> {
        self.element().static_pad("sink")
    }
}
