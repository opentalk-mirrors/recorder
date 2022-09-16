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
pub struct Layout {
    /// size of the output
    pub size: Size,
    /// positions of the viewers
    pub viewers_positions: Vec<Position>,
    /// size of a viewer
    pub viewers_size: Size,
    /// speaker's y-position
    pub speaker_position: Position,
    /// width of the speaker
    pub speaker_size: Size,
    /// alignment of the title
    pub title_alignment: Alignment,
    /// position of the title
    pub title_position: Position,
    /// position of the "who's speaking" text
    pub speaking_position: Position,
    /// alignment of the "who's speaking" text
    pub speaking_alignment: Alignment,
    /// position of the clock display
    pub clock_position: Position,
    /// alignment of the clock display
    pub clock_alignment: Alignment,
}

impl Layout {
    /// return the number of viewers that have to be displayed
    pub fn num_viewers(&self) -> usize {
        self.viewers_positions.len()
    }
    #[allow(dead_code)]
    pub fn no_viewers(&self) -> bool {
        self.viewers_positions.len() == 0
    }
}
