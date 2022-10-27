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
    pub fn ratio(&self) -> f64 {
        self.width as f64 / self.height as f64
    }
}

/// text alignment
#[derive(Debug, Clone)]
pub struct Alignment {
    /// horizontal alignment
    pub horizontal: &'static str,
    /// vertical alignment
    pub vertical: &'static str,
}

/// recording picture layout
pub trait Layout: Send + Sync + 'static {
    fn new(resolution: &Size) -> Self;
    fn resolution(&self) -> &Size;
    fn position(&self, n: usize, count: usize) -> Position;
    fn size(&self, n: usize, count: usize) -> Size;
    fn title_alignment(&self) -> Alignment;
    fn title_position(&self, _count: usize) -> Position;
    // align the "who's speaking" text
    fn speaking_alignment(&self, count: usize) -> Alignment;
    // place "who's speaking" text
    fn speaking_position(&self, count: usize) -> Position;
    // align clock display
    fn clock_alignment(&self) -> Alignment;
    fn clock_position(&self, count: usize) -> Position;
}
