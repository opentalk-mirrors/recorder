use super::*;

/// Grid layout
/// Places all the *visible* participants in a grid on screen.
#[derive(Clone)]
pub struct Grid {
    visibles: usize,
    // Size of the target picture in pixels.
    resolution: Size,
}

impl Layout for Grid {
    const NAME: &'static str = "grid";

    fn new(visibles: usize, resolution: Size) -> Self {
        Self {
            visibles,
            resolution,
        }
    }

    fn view(&self, n: usize) -> View {
        let row = n / self.columns();
        let column = n % self.columns();
        View {
            pos: Position {
                x: (self.width() * column) as i64,
                y: (self.height() * row + self.padding()) as i64,
            },
            size: self.uni_size(),
            alpha: 1.0,
        }
    }
}

impl Grid {
    fn columns(&self) -> usize {
        self.grid().0
    }

    fn rows(&self) -> usize {
        self.grid().1
    }

    fn grid(&self) -> (usize, usize) {
        if self.visibles > 1 {
            let columns = (f64::sqrt(self.visibles as f64) + 0.9) as usize;
            let rows = (self.visibles + columns - 1) / columns;
            if rows > columns {
                (columns + 1, rows - 1)
            } else {
                (columns, rows)
            }
        } else {
            (1, 1)
        }
    }

    fn width(&self) -> usize {
        self.uni_size().width
    }

    fn height(&self) -> usize {
        self.uni_size().height
    }

    fn uni_size(&self) -> Size {
        let width = self.resolution.width / self.columns();
        let height = (width as f64 / self.resolution.ratio()) as usize;
        Size { width, height }
    }

    fn padding(&self) -> usize {
        (self.resolution.height - self.height() * self.rows()) / 2
    }
}
