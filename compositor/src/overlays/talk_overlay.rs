// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use anyhow::{Context, Result};
use gst_base::prelude::*;

use crate::{
    Align, ClockOverlay, Font, HAlign, Overlay, Padding, PaddingOverlay, TextOverlay, TextPadding,
    TextStyle, VAlign,
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
    fn show(&self, show: bool) {
        self.text_overlay.show(show);
        self.clock_overlay.show(show);
    }
    #[must_use]
    fn sink(&self) -> Option<gst::Pad> {
        self.text_overlay.sink()
    }
    #[must_use]
    fn src(&self) -> Option<gst::Pad> {
        self.clock_overlay.src()
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
    pub fn create() -> Result<Self> {
        let bin = gst::Bin::new(Some("Talk Overlay"));
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
            "%x %X %Z",
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

        bin.add_many(&[
            padding_overlay.element(),
            text_overlay.element(),
            clock_overlay.element(),
        ])
        .context("unable to add padding_overlay, text_overlay and clock_overlay to the bin")?;

        gst::Element::link_many(&[
            padding_overlay.element(),
            text_overlay.element(),
            clock_overlay.element(),
        ])
        .context("unable to link padding_overlay, text_overlay and clock_overlay together")?;

        let padding_overlay_sink = padding_overlay
            .sink()
            .context("unable to get sink for padding_overlay")?;
        let video_sink = gst::GhostPad::with_target(Some("video_sink"), &padding_overlay_sink)
            .context("failed to create video ghost pad for participant overlay sink")?;
        bin.add_pad(&video_sink)
            .context("failed to add video ghost pad to participant overlay sink bin")?;
        let clock_overlay_src = &clock_overlay
            .src()
            .context("unable to get src for clock_overlay")?;
        let video_src = gst::GhostPad::with_target(Some("src"), clock_overlay_src)
            .context("failed to create video ghost pad for participant overlay sink")?;
        bin.add_pad(&video_src)
            .context("failed to add video ghost pad to participant overlay sink bin")?;

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
