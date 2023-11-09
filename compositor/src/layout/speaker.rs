// SPDX-FileCopyrightText: OpenTalk GmbH <mail@opentalk.eu>
//
// SPDX-License-Identifier: EUPL-1.2

use super::{Layout, Position, Size, View};

/// Speaker layout
#[derive(Debug, Default, Clone)]
pub struct Speaker {
    // Size of the target picture in pixels.
    resolution: Size,
    visibles: usize,
}

const VIEWER_SCALE: usize = 4;

impl Layout for Speaker {
    fn set_resolution_changed(&mut self, resolution: Size) {
        self.resolution = resolution;
    }

    fn set_amount_of_visibles(&mut self, visibles: usize) {
        self.visibles = visibles;
    }

    fn calculate_stream_view(&self, stream_position: usize) -> Option<View> {
        if stream_position >= self.visibles {
            return None;
        }
        let view = match stream_position {
            0 => View {
                pos: self.speaker_position(),
                size: self.speaker_size(),
            },
            _ => View {
                pos: self.viewers_position(stream_position - 1),
                size: self.viewers_size(),
            },
        };
        Some(view)
    }
}

impl Speaker {
    fn ratio(&self) -> f64 {
        self.resolution.width as f64 / self.resolution.height as f64
    }

    fn viewers_height(&self) -> usize {
        (self.viewers_width() as f64 / self.ratio()) as usize
    }

    fn viewers_width(&self) -> usize {
        match self.visibles {
            0 | 1 => 0,
            2 => self.resolution.width / 2,
            _ => self.resolution.width / VIEWER_SCALE,
        }
    }

    fn speaker_size(&self) -> Size {
        Size {
            height: self.speaker_height(),
            width: self.speaker_width(),
        }
    }

    fn speaker_height(&self) -> usize {
        self.resolution.height - self.viewers_height()
    }

    fn speaker_width(&self) -> usize {
        (self.speaker_height() as f64 * self.ratio()) as usize
    }

    fn viewers_position(&self, stream_position: usize) -> Position {
        // calculate viewers' positions
        match self.visibles {
            0 | 1 => Position { x: 0, y: 0 },
            // place one viewer centered beside the speaker
            2 => Position {
                x: self.resolution.width as i64 / 2,
                y: self.resolution.height as i64 / 4,
            },
            // otherwise arrange viewers at the right side of the picture
            _ => {
                if stream_position < VIEWER_SCALE {
                    Position {
                        x: self.speaker_width() as i64,
                        y: (self.viewers_height() * stream_position) as i64,
                    }
                } else {
                    // All the viewers where `n < VIEWER_SCALE` are placed on the right side of the column.
                    // The entire right column is filled with participants.
                    // That's the reason why there is an offset by 1, to avoid overlapping of two participants.
                    const HORIZONTAL_INDEX_OFFSET: usize = 1;
                    let horizontal_index = stream_position - VIEWER_SCALE + HORIZONTAL_INDEX_OFFSET;
                    let horizontal_offset = (self.viewers_width() * horizontal_index) as i64;
                    Position {
                        x: self.speaker_width() as i64 - horizontal_offset,
                        y: self.speaker_height() as i64,
                    }
                }
            }
        }
    }

    fn viewers_size(&self) -> Size {
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
            // place speaker beside the viewer arrangement and leave space at the bottom
            _ => Position { x: 0, y: 0 },
        }
    }
}
