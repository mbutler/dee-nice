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

use dee_nice_core::{BindingAction, Show};

use crate::media;

/// Why display mode cannot start. The first matching reason wins.
pub fn readiness(show: &Show, connected_names: &[String]) -> Result<(), String> {
    let directory = show.media_directory();
    if !directory.is_dir() {
        return Err(format!(
            "Choose a folder of videos. {} is not a folder.",
            directory.display()
        ));
    }
    let Some(display) = show.output_display() else {
        return Err("Choose the display that should go fullscreen.".to_string());
    };
    if !connected_names.iter().any(|name| name == display.name()) {
        return Err(format!(
            "The saved display \"{}\" is not connected. Choose another.",
            display.name()
        ));
    }
    let mut missing = Vec::new();
    let assigned = show.assignments().count();
    for (key, clip) in show.clips() {
        if let Err(error) = media::resolve_clip(directory, clip) {
            missing.push(format!("{} ({})", clip, key.label()));
            let _ = error;
        }
    }
    if assigned == 0 {
        return Err("Assign at least one key.".to_string());
    }
    if !missing.is_empty() {
        return Err(format!(
            "These assigned files are missing: {}.",
            missing.join(", ")
        ));
    }
    Ok(())
}

/// Drops a saved display that is not in `connected_names`.
///
/// Enumeration failure must not call this. An empty list means the saved
/// display is gone, and the shell then asks for another.
pub fn forget_missing_display(show: &mut Show, connected_names: &[String]) {
    let missing = show
        .output_display()
        .is_some_and(|display| !connected_names.iter().any(|name| name == display.name()));
    if missing {
        show.set_output_display(None);
    }
}

pub struct OpeningCue {
    pub label: String,
    pub key_name: String,
    /// `None` starts on black.
    pub clip: Option<String>,
    pub slot: dee_nice_core::Slot,
}

/// The first assignment in each slot, in key order, when the show starts locked.
pub fn opening_cues(show: &Show) -> Vec<OpeningCue> {
    if !show.start_locked() {
        return Vec::new();
    }
    let mut seen = std::collections::BTreeSet::new();
    show.playback_bindings(false)
        .into_iter()
        .filter_map(|binding| {
            let label = binding.chord.key().label();
            let key_name = binding.chord.key().name();
            let cue = match binding.action {
                BindingAction::Play { clip, slot } => OpeningCue {
                    label,
                    key_name,
                    clip: Some(clip),
                    slot,
                },
                BindingAction::Black { slot } => OpeningCue {
                    label,
                    key_name,
                    clip: None,
                    slot,
                },
                BindingAction::ToggleLock | BindingAction::ExitDisplay => return None,
            };
            seen.insert(cue.slot).then_some(cue)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use dee_nice_core::{Key, OutputDisplay, Show, Slot};
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let tick = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0);
            let n = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("dee-nice-ready-{tick}-{n}"));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn key(name: &str) -> Key {
        Key::from_name(name).unwrap()
    }

    #[test]
    fn a_ready_show_can_enter_and_a_gap_cannot() {
        let dir = TempDir::new();
        fs::write(dir.0.join("a.mp4"), b"video").unwrap();
        let mut show = Show::new(&dir.0).unwrap();
        let displays = vec!["Studio".to_string()];
        assert!(
            readiness(&show, &displays)
                .unwrap_err()
                .contains("Choose the display")
        );
        show.set_output_display(Some(OutputDisplay::new("Studio", 0).unwrap()));
        assert!(
            readiness(&show, &displays)
                .unwrap_err()
                .contains("at least one")
        );
        show.assign(key("b"), "b.mp4", Slot::Full).unwrap();
        assert!(readiness(&show, &displays).unwrap_err().contains("b.mp4"));
        show.clear(key("b"));
        show.assign(key("a"), "a.mp4", Slot::Full).unwrap();
        assert!(readiness(&show, &displays).is_ok());
        assert!(
            readiness(&show, &["Other".to_string()])
                .unwrap_err()
                .contains("not connected")
        );
        forget_missing_display(&mut show, &["Other".to_string()]);
        assert!(show.output_display().is_none());
        assert!(
            readiness(&show, &["Other".to_string()])
                .unwrap_err()
                .contains("Choose the display")
        );
    }

    #[test]
    fn start_locked_names_the_first_assigned_clip() {
        let mut show = Show::new(Path::new("/shows/opening")).unwrap();
        show.assign(key("b"), "b.mp4", Slot::Full).unwrap();
        show.assign(key("a"), "a.mp4", Slot::Full).unwrap();
        assert!(opening_cues(&show).is_empty());
        show.set_start_locked(true);
        let cue = opening_cues(&show).into_iter().next().unwrap();
        assert_eq!(cue.label, "A");
        assert_eq!(cue.clip.as_deref(), Some("a.mp4"));
        show.assign_black(key("a"), Slot::Full).unwrap();
        let black = opening_cues(&show).into_iter().next().unwrap();
        assert_eq!(black.key_name, "a");
        assert!(black.clip.is_none());
    }
}
