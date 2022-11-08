use super::*;

/// Speaker layout
#[derive(Clone)]
pub struct Speaker {
    // Size of the target picture in pixels.
    size: Size,
}

impl Layout for Speaker {
    /// create a layout where the viewers are vertically distributed at the right side
    /// of the speaker and remaining space is used to display a title and 'who's speaking'
    /// # Arguments
    /// - `resolution` : dimensions of the output picture in pixels
    /// # Return
    /// Returns a `Layout` instance you can use to call `Mixer::new_speaker()`.
    fn new(resolution: &Size) -> Self {
        // calculate layout
        Self {
            // overall picture size
            size: *resolution,
        }
    }

    fn resolution(&self) -> &Size {
        &self.size
    }

    fn position(&self, n: usize, count: usize) -> Position {
        match n {
            0 => self.speaker_position(count),
            _ => self.viewers_position(n - 1, count),
        }
    }

    fn size(&self, n: usize, count: usize) -> Size {
        match n {
            0 => self.speaker_size(count),
            _ => self.viewers_size(n - 1, count),
        }
    }

    fn title_alignment(&self) -> Alignment {
        // align the title text
        Alignment {
            horizontal: "left",
            vertical: "top",
        }
    }

    fn title_position(&self, _count: usize) -> Position {
        // place the title at the top left corner
        Position { x: 0, y: 0 }
    }

    fn speaking_alignment(&self, _count: usize) -> Alignment {
        Alignment {
            horizontal: "left",
            vertical: "bottom",
        }
    }

    fn speaking_position(&self, count: usize) -> Position {
        let pos = self.speaker_position(count);
        let size = self.speaker_size(count);
        let res = self.resolution();
        Position {
            x: pos.x,
            y: -(res.height as i64 - (size.height as i64 + pos.y)),
        }
    }

    fn clock_alignment(&self) -> Alignment {
        Alignment {
            horizontal: "right",
            vertical: "bottom",
        }
    }

    fn clock_position(&self, count: usize) -> Position {
        // place clock display
        Position {
            x: match count {
                // right within whole picture
                0 | 1 | 2 => 0,
                // right within title area
                _ => -(self.viewers_width(count) as i64),
            },
            y: match count {
                // bottom of the whole picture
                0 | 1 | 2 => 0,
                // bottom within title area
                _ => -(self.speaker_height(count) as i64),
            },
        }
    }
}
impl Speaker {
    fn ratio(&self) -> f64 {
        self.size.width as f64 / self.size.height as f64
    }

    fn viewers_height(&self, count: usize) -> usize {
        match count {
            0 | 1 => 0,
            2 => self.size.height / 2,
            _ => self.size.height / (count - 1),
        }
    }

    fn viewers_width(&self, count: usize) -> usize {
        (self.viewers_height(count) as f64 * self.ratio()) as usize
    }

    fn speaker_size(&self, count: usize) -> Size {
        Size {
            height: self.size.height - self.viewers_height(count),
            width: (self.speaker_height(count) as f64 * self.ratio()) as usize,
        }
    }

    fn speaker_height(&self, count: usize) -> usize {
        self.size.height - self.viewers_height(count)
    }

    fn speaker_width(&self, count: usize) -> usize {
        (self.speaker_height(count) as f64 * self.ratio()) as usize
    }

    fn viewers_position(&self, n: usize, count: usize) -> Position {
        // calculate viewers' positions
        match count {
            0 | 1 => Position { x: 0, y: 0 },
            // place one viewer centered beside the speaker
            2 => Position {
                x: self.size.width as i64 / 2,
                y: self.size.height as i64 / 4,
            },
            // otherwise arrange viewers at the right side of the picture
            _ => Position {
                x: self.speaker_width(count) as i64,
                y: (self.viewers_height(count) * n) as i64,
            },
        }
    }

    fn viewers_size(&self, _n: usize, count: usize) -> Size {
        // calculate viewers' size
        match count {
            // fit one viewer beside the speaker
            1 => Size {
                width: self.size.width / 2,
                height: self.size.height / 2,
            },
            // otherwise use viewers' size
            _ => Size {
                width: self.viewers_width(count),
                height: self.viewers_height(count),
            },
        }
    }

    fn speaker_position(&self, count: usize) -> Position {
        // calculate speaker's position
        match count {
            // place speaker beside single viewer
            2 => Position {
                x: 0,
                y: self.size.height as i64 / 4,
            },
            // place speaker beside the viewer arrangement and leave space at top
            _ => Position {
                x: 0,
                y: self.size.height as i64 - self.speaker_height(count) as i64,
            },
        }
    }
}
