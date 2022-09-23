/// Cartesian pixel position
#[derive(Debug, Clone)]
pub struct Position {
    /// X position
    pub x: i64,
    /// Y position
    pub y: i64,
}

/// Cartesian pixel dimension
#[derive(Debug, Clone)]
pub struct Size {
    /// horizontal dimension
    pub width: usize,
    /// vertical dimension
    pub height: usize,
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
pub trait Layout {
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
