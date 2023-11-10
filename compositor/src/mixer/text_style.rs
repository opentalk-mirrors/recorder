// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

//! Text styles.

/// Text color.
#[derive(Debug)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Default for Color {
    fn default() -> Color {
        Color {
            r: 0xff,
            g: 0xff,
            b: 0xff,
            a: 0xff,
        }
    }
}

impl From<Color> for u32 {
    fn from(color: Color) -> u32 {
        (u32::from(color.r)) << 24
            | (u32::from(color.g)) << 16
            | (u32::from(color.b)) << 8
            | (u32::from(color.a))
    }
}
impl From<Color> for glib::Value {
    fn from(color: Color) -> glib::Value {
        glib::Value::from(
            (u32::from(color.a)) << 24
                | (u32::from(color.b)) << 16
                | (u32::from(color.g)) << 8
                | (u32::from(color.r)),
        )
    }
}

impl std::fmt::Display for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#{:x}{:x}{:x}{:x}", self.r, self.g, self.b, self.a)
    }
}

/// Text padding.
#[derive(Debug)]
pub struct TextPadding {
    pub x: i32,
    pub y: i32,
}

impl Default for TextPadding {
    fn default() -> TextPadding {
        TextPadding { x: 10, y: 10 }
    }
}

impl std::fmt::Display for TextPadding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{x}/{y}", x = self.x, y = self.y)
    }
}

/// Text font.
#[derive(Debug)]
pub struct Font {
    pub name: &'static str,
    pub size: u32,
}

impl Default for Font {
    fn default() -> Font {
        Font {
            name: "Sans",
            size: 10,
        }
    }
}

impl std::fmt::Display for Font {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{},{}", self.name, self.size)
    }
}

/// Horizontal text alignment.
#[derive(Debug, Default)]
pub enum HAlign {
    Left,
    #[default]
    Center,
    Right,
    Position,
    Absolute,
}

impl From<HAlign> for &'static str {
    fn from(align: HAlign) -> &'static str {
        align.as_str()
    }
}

impl std::fmt::Display for HAlign {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{halign}", halign = self.as_str())
    }
}

impl HAlign {
    fn as_str(&self) -> &'static str {
        match self {
            HAlign::Left => "left",
            HAlign::Center => "center",
            HAlign::Right => "right",
            HAlign::Position => "position",
            HAlign::Absolute => "absolute",
        }
    }
}

/// Vertical text alignment.
#[derive(Debug, Default)]
pub enum VAlign {
    #[default]
    Baseline,
    Bottom,
    Top,
    Position,
    Center,
    Absolute,
}

impl From<VAlign> for &'static str {
    fn from(align: VAlign) -> &'static str {
        align.as_str()
    }
}

impl std::fmt::Display for VAlign {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{valign}", valign = self.as_str())
    }
}

impl VAlign {
    fn as_str(&self) -> &'static str {
        match self {
            VAlign::Baseline => "baseline",
            VAlign::Bottom => "bottom",
            VAlign::Top => "top",
            VAlign::Position => "position",
            VAlign::Center => "center",
            VAlign::Absolute => "absolute",
        }
    }
}

/// Horizontal and vertical text alignment.
#[derive(Debug, Default)]
pub struct Align {
    pub horizontal: HAlign,
    pub vertical: VAlign,
}

impl std::fmt::Display for Align {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{horizontal} × {vertical}",
            horizontal = self.horizontal,
            vertical = self.vertical
        )
    }
}

/// Text format.
#[derive(Debug, Default)]
pub struct TextStyle {
    pub font: Font,
    pub padding: TextPadding,
    pub color: Color,
    pub align: Align,
}

impl std::fmt::Display for TextStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{font}, {padding}, {color}, {align}",
            font = self.font,
            padding = self.padding,
            color = self.color,
            align = self.align
        )
    }
}
