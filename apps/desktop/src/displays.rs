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

/// A connected display. `name` is the localized screen name mpv matches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttachedDisplay {
    pub name: String,
    pub label: String,
    pub index: u32,
    pub primary: bool,
    /// Long edge in pixels. `display-info` reports points in `width` and `height`.
    pub pixel_long_edge: u32,
    /// Origin and size in points, for placing the operator window.
    pub origin_x: i32,
    pub origin_y: i32,
    pub point_width: u32,
    pub point_height: u32,
}

pub fn attached_displays() -> Result<Vec<AttachedDisplay>, String> {
    let mut infos = display_info::DisplayInfo::all().map_err(|error| error.to_string())?;
    infos.sort_by(|left, right| {
        right
            .is_primary
            .cmp(&left.is_primary)
            .then_with(|| left.friendly_name.cmp(&right.friendly_name))
            .then_with(|| left.id.cmp(&right.id))
    });
    let displays = infos
        .into_iter()
        .enumerate()
        .map(|(index, info)| {
            let name = if info.friendly_name.is_empty() {
                format!("Display {}", info.id)
            } else {
                info.friendly_name
            };
            let pixel_width = scale_points(info.width, info.scale_factor);
            let pixel_height = scale_points(info.height, info.scale_factor);
            let role = if info.is_primary { ", main" } else { "" };
            let label = format!("{name}{role} — {pixel_width}×{pixel_height}");
            AttachedDisplay {
                name,
                label,
                index: u32::try_from(index).unwrap_or(u32::MAX),
                primary: info.is_primary,
                pixel_long_edge: pixel_width.max(pixel_height),
                origin_x: info.x,
                origin_y: info.y,
                point_width: info.width,
                point_height: info.height,
            }
        })
        .collect();
    Ok(displays)
}

/// The display that should keep the setup window during a show.
///
/// Kiosk mode returns nothing, so the window hides. Presentation mode uses
/// another connected display, preferring the main one. A single display hides
/// the window too, because there is nowhere else to put it.
pub fn operator_display<'a>(
    presentation: bool,
    output_name: &str,
    displays: &'a [AttachedDisplay],
) -> Option<&'a AttachedDisplay> {
    if !presentation {
        return None;
    }
    displays
        .iter()
        .find(|display| display.primary && display.name != output_name)
        .or_else(|| displays.iter().find(|display| display.name != output_name))
}

fn scale_points(points: u32, scale: f32) -> u32 {
    if !scale.is_finite() || scale <= 0.0 {
        return points.max(1);
    }
    ((points as f32) * scale)
        .round()
        .clamp(1.0, u32::MAX as f32) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn display(name: &str, primary: bool) -> AttachedDisplay {
        AttachedDisplay {
            name: name.to_string(),
            label: name.to_string(),
            index: 0,
            primary,
            pixel_long_edge: 1920,
            origin_x: 0,
            origin_y: 0,
            point_width: 1920,
            point_height: 1080,
        }
    }

    #[test]
    fn kiosk_hides_the_window_and_presentation_uses_the_other_display() {
        let displays = vec![display("Built-in", true), display("Hall", false)];
        assert!(operator_display(false, "Hall", &displays).is_none());
        assert_eq!(
            operator_display(true, "Hall", &displays).map(|display| display.name.as_str()),
            Some("Built-in")
        );
        assert_eq!(
            operator_display(true, "Built-in", &displays).map(|display| display.name.as_str()),
            Some("Hall")
        );
        assert!(operator_display(true, "Built-in", &displays[..1]).is_none());
    }
}
