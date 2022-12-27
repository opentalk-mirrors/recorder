use super::*;

/// Speaker layout
#[derive(Clone)]
pub struct Speaker {
    visibles: usize,
    // Size of the target picture in pixels.
    resolution: Size,
}

impl Layout for Speaker {
    #[cfg(test)]
    const NAME: &'static str = "speaker";

    fn new(visibles: usize, resolution: Size) -> Self {
        Self {
            visibles,
            resolution,
        }
    }

    fn view(&self, n: usize) -> View {
        match n {
            0 => View {
                pos: self.speaker_position(),
                size: self.speaker_size(),
                alpha: 1.0,
            },
            _ => View {
                pos: self.viewers_position(n - 1),
                size: self.viewers_size(n - 1),
                alpha: 1.0,
            },
        }
    }
}

impl Speaker {
    fn ratio(&self) -> f64 {
        self.resolution.width as f64 / self.resolution.height as f64
    }

    fn viewers_height(&self) -> usize {
        match self.visibles {
            0 | 1 => 0,
            2 => self.resolution.height / 2,
            _ => self.resolution.height / (self.visibles - 1),
        }
    }

    fn viewers_width(&self) -> usize {
        (self.viewers_height() as f64 * self.ratio()) as usize
    }

    fn speaker_size(&self) -> Size {
        Size {
            height: self.resolution.height - self.viewers_height(),
            width: (self.speaker_height() as f64 * self.ratio()) as usize,
        }
    }

    fn speaker_height(&self) -> usize {
        self.resolution.height - self.viewers_height()
    }

    fn speaker_width(&self) -> usize {
        (self.speaker_height() as f64 * self.ratio()) as usize
    }

    fn viewers_position(&self, n: usize) -> Position {
        // calculate viewers' positions
        match self.visibles {
            0 | 1 => Position { x: 0, y: 0 },
            // place one viewer centered beside the speaker
            2 => Position {
                x: self.resolution.width as i64 / 2,
                y: self.resolution.height as i64 / 4,
            },
            // otherwise arrange viewers at the right side of the picture
            _ => Position {
                x: self.speaker_width() as i64,
                y: (self.viewers_height() * n) as i64,
            },
        }
    }

    fn viewers_size(&self, _n: usize) -> Size {
        // calculate viewers' size
        match self.visibles {
            // fit one viewer beside the speaker
            1 => Size {
                width: self.resolution.width / 2,
                height: self.resolution.height / 2,
            },
            // otherwise use viewers' size
            _ => Size {
                width: self.viewers_width(),
                height: self.viewers_height(),
            },
        }
    }

    fn speaker_position(&self) -> Position {
        // calculate speaker's position
        match self.visibles {
            // place speaker beside single viewer
            2 => Position {
                x: 0,
                y: self.resolution.height as i64 / 4,
            },
            // place speaker beside the viewer arrangement and leave space at top
            _ => Position {
                x: 0,
                y: self.resolution.height as i64 - self.speaker_height() as i64,
            },
        }
    }
}
