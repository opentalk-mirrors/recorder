use super::*;

/// Grid layout
/// Places all the *visible* participants in a grid on screen.
#[derive(Clone)]
pub struct Grid {
    // Size of the target picture in pixels.
    size: Size,
}

impl Layout for Grid {
    #[cfg(test)]
    fn name() -> &'static str {
        "Grid"
    }

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
        let row = n / self.columns(count);
        let column = n % self.columns(count);
        Position {
            x: (self.width(count) * column) as i64,
            y: (self.height(count) * row + self.padding(count)) as i64,
        }
    }

    fn size(&self, _n: usize, count: usize) -> Size {
        self.uni_size(count)
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
        let pos = self.position(0, count);
        let size = self.size(0, count);
        let res = self.resolution();
        Position {
            x: pos.x,
            y: -(res.height as i64 - (size.height as i64 + pos.y)),
        }
    }

    // align clock display
    fn clock_alignment(&self) -> Alignment {
        Alignment {
            horizontal: "right",
            vertical: "bottom",
        }
    }

    fn clock_position(&self, _count: usize) -> Position {
        // place clock display
        Position { x: 0, y: 0 }
    }
}

impl Grid {
    fn columns(&self, count: usize) -> usize {
        self.grid(count).0
    }

    fn rows(&self, count: usize) -> usize {
        self.grid(count).1
    }

    fn grid(&self, count: usize) -> (usize, usize) {
        if count > 1 {
            let columns = (f64::sqrt(count as f64) + 0.9) as usize;
            let rows = (count + columns - 1) / columns;
            if rows > columns {
                (columns + 1, rows - 1)
            } else {
                (columns, rows)
            }
        } else {
            (1, 1)
        }
    }

    fn width(&self, count: usize) -> usize {
        self.uni_size(count).width
    }

    fn height(&self, count: usize) -> usize {
        self.uni_size(count).height
    }

    fn uni_size(&self, count: usize) -> Size {
        let width = self.resolution().width / self.columns(count);
        let height = (width as f64 / self.resolution().ratio()) as usize;
        Size { width, height }
    }

    fn padding(&self, count: usize) -> usize {
        (self.resolution().height - self.height(count) * self.rows(count)) / 2
    }
}
