// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use super::{Layout, Position, Size, View};

/// Grid layout
/// Places all the *visible* participants in a grid on screen.
#[derive(Debug, Default, Clone)]
pub struct Grid {
    resolution: Size,
    visibles: usize,
}

impl Layout for Grid {
    fn set_resolution_changed(&mut self, resolution: Size) {
        self.resolution = resolution;
    }

    fn set_amount_of_visibles(&mut self, visibles: usize) {
        self.visibles = visibles;
    }

    fn calculate_stream_view(&self, stream_position: usize) -> Option<View> {
        let row = stream_position / self.columns();
        let column = stream_position % self.columns();
        Some(View {
            pos: Position {
                x: (self.width() * column) as i64,
                y: (self.height() * row + self.padding()) as i64,
            },
            size: self.uni_size(),
        })
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
