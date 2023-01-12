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
        (color.r as u32) << 24 | (color.g as u32) << 16 | (color.b as u32) << 8 | (color.a as u32)
    }
}

#[derive(Debug)]
pub struct Padding {
    pub x: i32,
    pub y: i32,
}

impl Default for Padding {
    fn default() -> Padding {
        Padding { x: 10, y: 10 }
    }
}

#[derive(Debug)]
pub struct Font {
    pub name: &'static str,
    pub size: u32,
}

impl Default for Font {
    fn default() -> Font {
        Font {
            name: "Sans",
            size: 14,
        }
    }
}

#[derive(Debug)]
pub enum HAlign {
    Left,
    Center,
    Right,
    Position,
    Absolute,
}

impl Default for HAlign {
    fn default() -> HAlign {
        HAlign::Center
    }
}

impl From<HAlign> for &'static str {
    fn from(align: HAlign) -> &'static str {
        match align {
            HAlign::Left => "left",
            HAlign::Center => "center",
            HAlign::Right => "right",
            HAlign::Position => "position",
            HAlign::Absolute => "absolute",
        }
    }
}

#[derive(Debug)]
pub enum VAlign {
    Baseline,
    Bottom,
    Top,
    Position,
    Center,
    Absolute,
}

impl Default for VAlign {
    fn default() -> VAlign {
        VAlign::Baseline
    }
}

impl From<VAlign> for &'static str {
    fn from(align: VAlign) -> &'static str {
        match align {
            VAlign::Baseline => "baseline",
            VAlign::Bottom => "bottom",
            VAlign::Top => "top",
            VAlign::Position => "position",
            VAlign::Center => "center",
            VAlign::Absolute => "absolute",
        }
    }
}

#[derive(Debug, Default)]
pub struct Align {
    pub horizontal: HAlign,
    pub vertical: VAlign,
}

#[derive(Debug, Default)]
pub struct TextFormat {
    pub font: Font,
    pub padding: Padding,
    pub color: Color,
    pub align: Align,
}
