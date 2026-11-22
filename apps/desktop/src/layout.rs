// Copyright (C) 2026 Matthew Butler
//
// dee-nice is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

use dee_nice_core::{Slot, Template};

/// A slot as fractions of the output screen. The origin is the top left.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Frame {
    pub fn mpv_geometry(self) -> String {
        format!(
            "{:.4}%x{:.4}%+{:.4}%+{:.4}%",
            self.width * 100.0,
            self.height * 100.0,
            self.x * 100.0,
            self.y * 100.0,
        )
    }
}

pub fn slot_frame(template: Template, slot: Slot, screen_aspect: f64) -> Frame {
    match template {
        Template::Full => Frame {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        },
        Template::SideBySide => side(slot),
        Template::Stacked => stack(slot),
        Template::Four => four(slot),
        Template::Portraits => portraits(slot, screen_aspect),
    }
}

fn side(slot: Slot) -> Frame {
    let x = if slot == Slot::Right { 0.5 } else { 0.0 };
    Frame {
        x,
        y: 0.0,
        width: 0.5,
        height: 1.0,
    }
}

fn stack(slot: Slot) -> Frame {
    let y = if slot == Slot::Bottom { 0.5 } else { 0.0 };
    Frame {
        x: 0.0,
        y,
        width: 1.0,
        height: 0.5,
    }
}

fn four(slot: Slot) -> Frame {
    let (x, y) = match slot {
        Slot::TopRight => (0.5, 0.0),
        Slot::BottomLeft => (0.0, 0.5),
        Slot::BottomRight => (0.5, 0.5),
        _ => (0.0, 0.0),
    };
    Frame {
        x,
        y,
        width: 0.5,
        height: 0.5,
    }
}

/// Two tall frames centered on the screen, with black around them.
fn portraits(slot: Slot, screen_aspect: f64) -> Frame {
    let aspect = if screen_aspect.is_finite() && screen_aspect > 0.0 {
        screen_aspect
    } else {
        16.0 / 9.0
    };
    let gap = 0.04;
    let portrait = 9.0 / 16.0;
    let mut height = 1.0;
    let mut width = height / aspect * portrait;
    let max_width = (1.0 - gap) / 2.0;
    if width > max_width {
        width = max_width;
        height = width * aspect / portrait;
    }
    let pair = width * 2.0 + gap;
    let origin_x = (1.0 - pair) / 2.0;
    let origin_y = (1.0 - height) / 2.0;
    let x = if slot == Slot::Right {
        origin_x + width + gap
    } else {
        origin_x
    };
    Frame {
        x,
        y: origin_y,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn side_by_side_splits_the_screen_in_half() {
        let left = slot_frame(Template::SideBySide, Slot::Left, 16.0 / 9.0);
        let right = slot_frame(Template::SideBySide, Slot::Right, 16.0 / 9.0);
        assert_eq!(left.mpv_geometry(), "50.0000%x100.0000%+0.0000%+0.0000%");
        assert_eq!(right.x, 0.5);
        assert_eq!(right.width, 0.5);
    }

    #[test]
    fn four_up_uses_the_corners() {
        let corner = slot_frame(Template::Four, Slot::BottomRight, 1.0);
        assert_eq!(corner.x, 0.5);
        assert_eq!(corner.y, 0.5);
    }

    #[test]
    fn portraits_stay_tall_and_centered_on_a_landscape_screen() {
        let left = slot_frame(Template::Portraits, Slot::Left, 16.0 / 9.0);
        let right = slot_frame(Template::Portraits, Slot::Right, 16.0 / 9.0);
        assert!(left.width < 0.4);
        assert!(left.height > 0.9);
        assert!(left.x > 0.1);
        assert!(right.x > left.x + left.width);
        assert!(left.x + left.width + right.width < 0.95);
    }
}
