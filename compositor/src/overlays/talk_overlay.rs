// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use gst_base::prelude::*;

use crate::{
    Align, ClockOverlay, Font, HAlign, Overlay, Padding, PaddingOverlay, TextOverlay, TextPadding,
    TextStyle, VAlign,
};

const TOP_PADDING: i32 = 56;
const OVERLAY_FONT_SIZE: u32 = 20;

/// Parameters of TalkOverlay
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
    fn element(&self) -> &gst::Element {
        self.bin.as_ref()
    }
    fn show(&self, show: bool) {
        self.text_overlay.show(show);
        self.clock_overlay.show(show);
    }
    fn sink(&self) -> gst::Pad {
        self.text_overlay.sink()
    }
    fn src(&self) -> gst::Pad {
        self.clock_overlay.src()
    }
}

impl TalkOverlay {
    /// Create and add new overlay sink into existing pipeline.
    pub fn new() -> Self {
        let bin = gst::Bin::new(Some("Talk Overlay"));
        let padding_overlay = PaddingOverlay::new(
            "padding",
            Padding {
                top: TOP_PADDING,
                ..Default::default()
            },
        );
        let text_overlay = TextOverlay::new(
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
        );
        let clock_overlay = ClockOverlay::new(
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
        );

        bin.add(padding_overlay.element())
            .expect("can not add padding overlay to video overlay bin");
        bin.add(text_overlay.element())
            .expect("can not add text overlay to video overlay bin");
        bin.add(clock_overlay.element())
            .expect("can not add clock overlay to video overlay bin");

        gst::Element::link_many(&[
            padding_overlay.element(),
            text_overlay.element(),
            clock_overlay.element(),
        ])
        .expect("cannot link participant overlay together");

        let video_sink = gst::GhostPad::with_target(Some("video_sink"), &padding_overlay.sink())
            .expect("failed to create video ghost pad for participant overlay sink");
        bin.add_pad(&video_sink)
            .expect("failed to add video ghost pad to participant overlay sink bin");
        let video_src = gst::GhostPad::with_target(Some("src"), &clock_overlay.src())
            .expect("failed to create video ghost pad for participant overlay sink");
        bin.add_pad(&video_src)
            .expect("failed to add video ghost pad to participant overlay sink bin");

        TalkOverlay {
            _padding_overlay: padding_overlay,
            text_overlay,
            clock_overlay,
            bin,
        }
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

impl Default for TalkOverlay {
    fn default() -> Self {
        Self::new()
    }
}
