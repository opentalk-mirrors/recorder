mod grid;
mod speaker;

pub use grid::*;
pub use speaker::*;

/// Cartesian pixel position
#[derive(Debug, Clone)]
pub struct Position {
    /// X position
    pub x: i64,
    /// Y position
    pub y: i64,
}

/// Cartesian pixel dimension
#[derive(Debug, Clone, Copy)]
pub struct Size {
    /// horizontal dimension
    pub width: usize,
    /// vertical dimension
    pub height: usize,
}

impl Size {
    /// SD (Standard Definition)
    pub const SD: Self = Self {
        width: 640,
        height: 480,
    };
    /// HD (High Definition)
    pub const HD: Self = Self {
        width: 720,
        height: 480,
    };
    /// Full HD (FHD)
    pub const FHD: Self = Self {
        width: 1920,
        height: 1080,
    };
    /// QHD (Quad HD)
    pub const QHD: Self = Self {
        width: 2560,
        height: 1440,
    };
    /// 4K video or Ultra HD (UHD)
    pub const UHD: Self = Self {
        width: 3840,
        height: 2160,
    };
    /// 8K video or Full Ultra HD
    pub const FULL_ULTRA_HD: Self = Self {
        width: 7680,
        height: 4320,
    };
    /// return ratio between width and height
    pub fn ratio(&self) -> f64 {
        self.width as f64 / self.height as f64
    }
}

/// Text alignment
#[derive(Debug, Clone)]
pub struct Alignment {
    /// horizontal alignment
    /// (see [this list](https://gstreamer.freedesktop.org/documentation/pango/GstBaseTextOverlay.html?gi-language=c#GstBaseTextOverlayHAlign) for possible values).
    pub horizontal: &'static str,
    /// vertical alignment
    /// (see [this list](https://gstreamer.freedesktop.org/documentation/pango/GstBaseTextOverlay.html?gi-language=c#GstBaseTextOverlayVAlign) for possible values).
    pub vertical: &'static str,
}

/// Video picture layout
pub trait Layout: Send + Sync + 'static {
    /// Create new layout for the given solution.
    fn new(resolution: &Size) -> Self;
    /// Get setup resolution.
    fn resolution(&self) -> &Size;
    /// Get position of the nth participants video.
    fn position(&self, n: usize, count: usize) -> Position;
    /// Get size of the nth participants video.
    fn size(&self, n: usize, count: usize) -> Size;
    /// Get alignment of the title text.
    fn title_alignment(&self) -> Alignment;
    /// Get position of the title text.
    fn title_position(&self, _count: usize) -> Position;
    /// Get alignment of the "who's speaking" text.
    fn speaking_alignment(&self, count: usize) -> Alignment;
    /// Get position of the "who's speaking" text.
    fn speaking_position(&self, count: usize) -> Position;
    /// Get alignment of the clock display.
    fn clock_alignment(&self) -> Alignment;
    /// Get position of the clock display.
    fn clock_position(&self, count: usize) -> Position;
}
