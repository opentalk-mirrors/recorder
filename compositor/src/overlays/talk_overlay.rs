// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use anyhow::{Context, Result};
use gst::{Element, GhostPad};
use serde::Deserialize;

use crate::{
    Align, ClockOverlay, Font, GstBinErrorExt, GstElementErrorExt, GstGhostPadErrorExt, HAlign,
    Overlay, Padding, PaddingOverlay, TextOverlay, TextPadding, TextStyle, VAlign,
};

const TOP_PADDING: i32 = 56;
const OVERLAY_FONT_SIZE: u32 = 20;

/// Parameters of `TalkOverlay`
#[allow(dead_code)]
pub struct TalkOverlaysParams {
    padding: TextPadding,
    // title invisible if `None`
    title_style: Option<TextStyle>,
    // clock invisible if `None`
    clock_style: Option<TextStyle>,
    // format string for clock display if visible
    clock_format: String,
}

/// Overlay which is used on top of a talk.
#[derive(Debug, Clone)]
pub struct TalkOverlay {
    _padding_overlay: PaddingOverlay,
    text_overlay: TextOverlay,
    clock_overlay: ClockOverlay,
    bin: gst::Bin,
}

impl Overlay for TalkOverlay {
    #[must_use]
    fn element(&self) -> &gst::Element {
        self.bin.as_ref()
    }
    #[must_use]
    fn sink(&self) -> Option<gst::Pad> {
        self.text_overlay.sink()
    }
    #[must_use]
    fn src(&self) -> Option<gst::Pad> {
        self.clock_overlay.src()
    }
    fn show(&self, show: bool) {
        self.text_overlay.show(show);
        self.clock_overlay.show(show);
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClockFormat(String);

impl Default for ClockFormat {
    fn default() -> Self {
        Self(String::from("%x %X %Z"))
    }
}

impl AsRef<str> for ClockFormat {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl TalkOverlay {
    /// Create and add new overlay sink into existing pipeline.
    ///
    /// # Errors
    ///
    /// This can fail for the following reasons:
    /// - The `PaddingOverlay` cannot be created.
    /// - The `TextOverlay` cannot be created.
    /// - The `ClockOverlay` cannot be created.
    /// - Adding the elements to Gstreamer or linking them.
    pub fn create(format: &ClockFormat) -> Result<Self> {
        let bin = gst::Bin::builder().name("Talk Overlay").build();
        let padding_overlay = PaddingOverlay::create(
            "padding",
            &Padding {
                top: TOP_PADDING,
                ..Default::default()
            },
        )
        .context("unable to create PaddingOverlay")?;
        let text_overlay = TextOverlay::create(
            "Title Overlay",
            "",
            TextStyle {
                align: Align {
                    horizontal: HAlign::Left,
                    vertical: VAlign::Top,
                },
                font: Font {
                    size: OVERLAY_FONT_SIZE,
                    ..Default::default()
                },
                ..Default::default()
            },
        )?;
        let clock_overlay = ClockOverlay::create(
            "Real Time Clock Overlay",
            format.as_ref(),
            TextStyle {
                align: Align {
                    horizontal: HAlign::Right,
                    vertical: VAlign::Top,
                },
                font: Font {
                    size: OVERLAY_FONT_SIZE,
                    ..Default::default()
                },
                ..Default::default()
            },
        )?;

        bin.add_many_with_context(&[
            padding_overlay.element(),
            text_overlay.element(),
            clock_overlay.element(),
        ])?;

        Element::link_many_with_context(&[
            padding_overlay.element(),
            text_overlay.element(),
            clock_overlay.element(),
        ])?;

        let padding_overlay_sink = padding_overlay
            .sink()
            .context("unable to get sink for padding_overlay")?;
        let video_sink =
            GhostPad::with_target_with_context(Some("video_sink"), &padding_overlay_sink)?;
        bin.add_pad_with_context(&video_sink)?;
        let clock_overlay_src = &clock_overlay
            .src()
            .context("unable to get src for clock_overlay")?;
        let video_src = GhostPad::with_target_with_context(Some("src"), clock_overlay_src)?;
        bin.add_pad_with_context(&video_src)?;

        Ok(Self {
            _padding_overlay: padding_overlay,
            text_overlay,
            clock_overlay,
            bin,
        })
    }
    pub fn set_title(&self, title: &str) {
        self.text_overlay.set(title);
    }
    pub fn show_title(&self, visible: bool) {
        self.text_overlay.show(visible);
    }
    pub fn show_clock(&self, visible: bool) {
        self.clock_overlay.show(visible);
    }
}
