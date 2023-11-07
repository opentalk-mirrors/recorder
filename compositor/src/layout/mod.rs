mod grid;
mod speaker;

pub use grid::*;
pub use speaker::*;

/// View properies of a stream
#[derive(Debug, Clone, Default)]
pub struct View {
    pub pos: Position,
    pub size: Size,
}

/// Cartesian pixel position
#[derive(Debug, Clone, Default)]
pub struct Position {
    /// X position
    pub x: i64,
    /// Y position
    pub y: i64,
}

/// Cartesian pixel dimension
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size {
    /// horizontal dimension
    pub width: usize,
    /// vertical dimension
    pub height: usize,
}

impl std::fmt::Display for Size {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}x{}", self.width, self.height)
    }
}

impl Size {
    /// SD (Standard Definition)
    pub const SD: Self = Self {
        width: 640,
        height: 480,
    };
    /// HD (High Definition)
    pub const HD: Self = Self {
        width: 1280,
        height: 720,
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
    /// Horizontal alignment
    /// (see [this list](https://gstreamer.freedesktop.org/documentation/pango/GstBaseTextOverlay.html?gi-language=c#GstBaseTextOverlayHAlign) for possible values).
    pub horizontal: &'static str,
    /// Vertical alignment
    /// (see [this list](https://gstreamer.freedesktop.org/documentation/pango/GstBaseTextOverlay.html?gi-language=c#GstBaseTextOverlayVAlign) for possible values).
    pub vertical: &'static str,
}

/// Trait of video picture layouts.
pub trait Layout: Send + Sync + 'static {
    /// create a new layout instance
    ///
    /// # Arguments
    ///
    /// - `visibles`: number of streams which shall be visible
    ///
    fn new(visibles: usize, resolution: Size) -> Self;
    /// Get view of the nth stream.
    ///
    /// # Arguments
    ///
    /// - `n`: index of the stream within this layout.
    ///
    /// Returns
    ///
    /// Returns Some(view) if the stream should be visible and be shown.
    /// Returns None if the stream should NOT be visible.
    fn view(&self, n: usize) -> Option<View>;

    /// Name of the layout
    const NAME: &'static str;
}
