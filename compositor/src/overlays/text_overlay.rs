// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

//! Overlay which displays a changeable text.

use anyhow::Result;
use gst::{prelude::*, ElementFactory};

use crate::{GstElementFactoryErrorExt, Overlay, TextStyle};

/// Text overlay.
#[derive(Debug, Clone)]
pub struct TextOverlay {
    element: gst::Element,
}

impl TextOverlay {
    /// Create new text overlay.
    ///
    /// # Arguments
    ///
    /// - `name`: Element's name.
    /// - `text`: Text to display.
    /// - `style`: Style of the text display.
    ///
    /// # Errors
    ///
    /// This can fail if the `textoverlay` cannot be created in `GStreamer`.
    pub fn create(name: &str, text: &str, style: TextStyle) -> Result<Self> {
        trace!("new( '{text}', {style:?} )");

        // create text overlay
        let element = ElementFactory::make_with_name_with_context("textoverlay", Some(name))?;

        // set up properties
        element.set_property("text", text);
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

    /// Change text to display.
    ///
    /// # Arguments
    ///
    /// - `text`: new text
    ///
    pub fn set(&self, text: &str) {
        trace!("set( '{text}' )");

        self.element.set_property("text", text);
    }
}

impl Overlay for TextOverlay {
    fn element(&self) -> &gst::Element {
        &self.element
    }
    fn show(&self, show: bool) {
        self.element.set_property("silent", !show);
    }
}
