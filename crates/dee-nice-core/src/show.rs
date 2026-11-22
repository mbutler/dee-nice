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

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use crate::error::ShowError;

/// A keyboard key that can start a clip.
///
/// The reserved lock and exit chords use modifiers, so the plain K and L keys
/// can still be assigned.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Key {
    kind: KeyKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum KeyKind {
    Letter(char),
    Digit(u8),
    Function(u8),
    Space,
    Left,
    Right,
    Up,
    Down,
}

impl Key {
    /// Parses a key name. Matching is case-insensitive.
    ///
    /// Canonical names are `a` through `z`, `0` through `9`, `f1` through
    /// `f12`, `space`, `left`, `right`, `up`, and `down`.
    pub fn from_name(name: &str) -> Option<Self> {
        let lower = name.trim().to_ascii_lowercase();
        let kind = if let Some(kind) = named_key(&lower) {
            kind
        } else if lower.len() == 1 {
            let ch = lower.chars().next()?;
            if ch.is_ascii_digit() {
                let value = ch.to_digit(10)?;
                KeyKind::Digit(u8::try_from(value).ok()?)
            } else if ch.is_ascii_lowercase() {
                KeyKind::Letter(ch)
            } else {
                return None;
            }
        } else {
            let number = function_key(&lower)?;
            KeyKind::Function(number)
        };
        Some(Self { kind })
    }

    fn letter(ch: char) -> Self {
        debug_assert!(ch.is_ascii_lowercase());
        Self {
            kind: KeyKind::Letter(ch),
        }
    }

    /// The stable name written into a show file.
    pub fn name(self) -> String {
        match self.kind {
            KeyKind::Letter(ch) => ch.to_string(),
            KeyKind::Digit(digit) => digit.to_string(),
            KeyKind::Function(number) => format!("f{number}"),
            KeyKind::Space => "space".to_string(),
            KeyKind::Left => "left".to_string(),
            KeyKind::Right => "right".to_string(),
            KeyKind::Up => "up".to_string(),
            KeyKind::Down => "down".to_string(),
        }
    }

    /// The label an operator sees on a key.
    pub fn label(self) -> String {
        match self.kind {
            KeyKind::Letter(ch) => ch.to_ascii_uppercase().to_string(),
            KeyKind::Digit(digit) => digit.to_string(),
            KeyKind::Function(number) => format!("F{number}"),
            KeyKind::Space => "Space".to_string(),
            KeyKind::Left => "Left".to_string(),
            KeyKind::Right => "Right".to_string(),
            KeyKind::Up => "Up".to_string(),
            KeyKind::Down => "Down".to_string(),
        }
    }
}

fn named_key(name: &str) -> Option<KeyKind> {
    Some(match name {
        "space" => KeyKind::Space,
        "left" => KeyKind::Left,
        "right" => KeyKind::Right,
        "up" => KeyKind::Up,
        "down" => KeyKind::Down,
        _ => return None,
    })
}

fn function_key(name: &str) -> Option<u8> {
    let rest = name.strip_prefix('f')?;
    let number = rest.parse::<u8>().ok()?;
    if (1..=12).contains(&number) && number.to_string() == rest {
        Some(number)
    } else {
        None
    }
}

/// A key plus the modifiers required to fire it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyChord {
    key: Key,
    ctrl: bool,
    shift: bool,
    alt: bool,
}

impl KeyChord {
    pub fn plain(key: Key) -> Self {
        Self {
            key,
            ctrl: false,
            shift: false,
            alt: false,
        }
    }

    /// Leaves display mode and returns to setup. Works while switching is locked.
    pub fn exit() -> Self {
        Self {
            key: Key::letter('k'),
            ctrl: true,
            shift: true,
            alt: false,
        }
    }

    /// Locks or unlocks clip switching without leaving display mode.
    pub fn toggle_lock() -> Self {
        Self {
            key: Key::letter('l'),
            ctrl: true,
            shift: true,
            alt: false,
        }
    }

    pub fn key(self) -> Key {
        self.key
    }

    pub fn ctrl(self) -> bool {
        self.ctrl
    }

    pub fn shift(self) -> bool {
        self.shift
    }

    pub fn alt(self) -> bool {
        self.alt
    }

    pub fn label(self) -> String {
        let mut parts = Vec::new();
        if self.ctrl {
            parts.push("Ctrl".to_string());
        }
        if self.alt {
            parts.push("Alt".to_string());
        }
        if self.shift {
            parts.push("Shift".to_string());
        }
        parts.push(self.key.label());
        parts.join("+")
    }
}

/// What display mode does when a chord is pressed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BindingAction {
    Play {
        clip: String,
        slot: Slot,
    },
    /// Cut one slot to black. The show stays open.
    Black {
        slot: Slot,
    },
    ToggleLock,
    ExitDisplay,
}

/// How one display is divided. Full is one picture, which is the default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Template {
    Full,
    SideBySide,
    Stacked,
    Four,
    Portraits,
}

impl Template {
    pub const ALL: [Template; 5] = [
        Self::Full,
        Self::SideBySide,
        Self::Stacked,
        Self::Four,
        Self::Portraits,
    ];

    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "full" => Self::Full,
            "side-by-side" => Self::SideBySide,
            "stacked" => Self::Stacked,
            "four" => Self::Four,
            "portraits" => Self::Portraits,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::SideBySide => "side-by-side",
            Self::Stacked => "stacked",
            Self::Four => "four",
            Self::Portraits => "portraits",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Full => "Full",
            Self::SideBySide => "Side by side",
            Self::Stacked => "Stacked",
            Self::Four => "Four",
            Self::Portraits => "Portraits",
        }
    }

    pub fn slots(self) -> &'static [Slot] {
        match self {
            Self::Full => &[Slot::Full],
            Self::SideBySide | Self::Portraits => &[Slot::Left, Slot::Right],
            Self::Stacked => &[Slot::Top, Slot::Bottom],
            Self::Four => &[
                Slot::TopLeft,
                Slot::TopRight,
                Slot::BottomLeft,
                Slot::BottomRight,
            ],
        }
    }

    pub fn is_full(self) -> bool {
        matches!(self, Self::Full)
    }
}

/// One region of the template. A key plays into one slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Slot {
    Full,
    Left,
    Right,
    Top,
    Bottom,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl Slot {
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "full" => Self::Full,
            "left" => Self::Left,
            "right" => Self::Right,
            "top" => Self::Top,
            "bottom" => Self::Bottom,
            "top-left" => Self::TopLeft,
            "top-right" => Self::TopRight,
            "bottom-left" => Self::BottomLeft,
            "bottom-right" => Self::BottomRight,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Left => "left",
            Self::Right => "right",
            Self::Top => "top",
            Self::Bottom => "bottom",
            Self::TopLeft => "top-left",
            Self::TopRight => "top-right",
            Self::BottomLeft => "bottom-left",
            Self::BottomRight => "bottom-right",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Full => "Full",
            Self::Left => "Left",
            Self::Right => "Right",
            Self::Top => "Top",
            Self::Bottom => "Bottom",
            Self::TopLeft => "Top left",
            Self::TopRight => "Top right",
            Self::BottomLeft => "Bottom left",
            Self::BottomRight => "Bottom right",
        }
    }

    /// A short mark drawn on a key. Full has none.
    pub fn mark(self) -> &'static str {
        match self {
            Self::Full => "",
            Self::Left => "L",
            Self::Right => "R",
            Self::Top => "T",
            Self::Bottom => "B",
            Self::TopLeft => "TL",
            Self::TopRight => "TR",
            Self::BottomLeft => "BL",
            Self::BottomRight => "BR",
        }
    }
}

/// One input binding handed to the playback shell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlaybackBinding {
    pub chord: KeyChord,
    pub action: BindingAction,
}

/// The monitor that should go fullscreen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutputDisplay {
    pub(crate) name: String,
    pub(crate) index: u32,
}

impl OutputDisplay {
    pub fn new(name: impl Into<String>, index: u32) -> Result<Self, ShowError> {
        let name = name.into();
        if name.is_empty() {
            return Err(ShowError::EmptyDisplayName);
        }
        Ok(Self { name, index })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn index(&self) -> u32 {
        self.index
    }
}

/// The setup window offers this interval when Auto-lock is turned on.
pub const AUTO_LOCK_SECONDS: u32 = 60;
const MAX_AUTO_LOCK_SECONDS: u32 = 24 * 60 * 60;
const MAX_KEY_NAME_CHARS: usize = 24;

/// One key in the show: a clip or black, plus the short name on that key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Assignment {
    target: AssignmentTarget,
    name: Option<String>,
    slot: Slot,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum AssignmentTarget {
    Clip(String),
    Black,
}

impl Assignment {
    pub fn clip(&self) -> Option<&str> {
        match &self.target {
            AssignmentTarget::Clip(clip) => Some(clip.as_str()),
            AssignmentTarget::Black => None,
        }
    }

    pub fn is_black(&self) -> bool {
        matches!(self.target, AssignmentTarget::Black)
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn slot(&self) -> Slot {
        self.slot
    }
}

/// The durable show: a media folder, one assignment per key, and playback settings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Show {
    pub(crate) media_directory: PathBuf,
    pub(crate) assignments: BTreeMap<Key, Assignment>,
    pub(crate) muted: bool,
    pub(crate) start_locked: bool,
    /// `None` leaves clip keys unlocked until the operator locks them.
    pub(crate) auto_lock_seconds: Option<u32>,
    pub(crate) template: Template,
    pub(crate) output_display: Option<OutputDisplay>,
}

impl Show {
    pub fn new(media_directory: impl Into<PathBuf>) -> Result<Self, ShowError> {
        Ok(Self {
            media_directory: require_directory(media_directory.into())?,
            assignments: BTreeMap::new(),
            muted: false,
            start_locked: false,
            auto_lock_seconds: None,
            template: Template::Full,
            output_display: None,
        })
    }

    pub fn media_directory(&self) -> &Path {
        &self.media_directory
    }

    pub fn set_media_directory(&mut self, directory: impl Into<PathBuf>) -> Result<(), ShowError> {
        self.media_directory = require_directory(directory.into())?;
        Ok(())
    }

    pub fn muted(&self) -> bool {
        self.muted
    }

    pub fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
    }

    /// Whether display mode should start with clip switching locked.
    ///
    /// This is not the live lock. [`Self::playback_bindings`] takes the live
    /// session value so a chord can change it without rewriting the show.
    pub fn start_locked(&self) -> bool {
        self.start_locked
    }

    pub fn set_start_locked(&mut self, start_locked: bool) {
        self.start_locked = start_locked;
    }

    /// Seconds of silence before clip keys lock again. `None` means off.
    pub fn auto_lock_seconds(&self) -> Option<u32> {
        self.auto_lock_seconds
    }

    /// `None` turns auto-lock off. A rejected value leaves the previous setting.
    pub fn set_auto_lock_seconds(&mut self, seconds: Option<u32>) -> Result<(), ShowError> {
        match seconds {
            None => self.auto_lock_seconds = None,
            Some(seconds) if (1..=MAX_AUTO_LOCK_SECONDS).contains(&seconds) => {
                self.auto_lock_seconds = Some(seconds);
            }
            Some(_) => return Err(ShowError::InvalidAutoLock),
        }
        Ok(())
    }

    pub fn template(&self) -> Template {
        self.template
    }

    /// Switches layout. A key whose slot disappeared moves to the first slot.
    pub fn set_template(&mut self, template: Template) {
        self.template = template;
        let fallback = template.slots()[0];
        for assignment in self.assignments.values_mut() {
            if !template.slots().contains(&assignment.slot) {
                assignment.slot = fallback;
            }
        }
    }

    pub fn output_display(&self) -> Option<&OutputDisplay> {
        self.output_display.as_ref()
    }

    pub fn set_output_display(&mut self, display: Option<OutputDisplay>) {
        self.output_display = display;
    }

    pub fn assignment(&self, key: Key) -> Option<&Assignment> {
        self.assignments.get(&key)
    }

    pub fn clip(&self, key: Key) -> Option<&str> {
        self.assignment(key).and_then(Assignment::clip)
    }

    pub fn is_black(&self, key: Key) -> bool {
        self.assignment(key).is_some_and(Assignment::is_black)
    }

    pub fn key_name(&self, key: Key) -> Option<&str> {
        self.assignment(key).and_then(Assignment::name)
    }

    pub fn assignments(&self) -> impl Iterator<Item = (Key, &Assignment)> + '_ {
        self.assignments
            .iter()
            .map(|(key, assignment)| (*key, assignment))
    }

    pub fn clips(&self) -> impl Iterator<Item = (Key, &str)> + '_ {
        self.assignments()
            .filter_map(|(key, assignment)| assignment.clip().map(|clip| (key, clip)))
    }

    /// Assigns a clip inside the media directory. A new assignment replaces the old one.
    ///
    /// The clip path is relative. Absolute paths and `..` are rejected, and a
    /// rejected assignment leaves the show unchanged. Replacing the clip clears
    /// its name. Assigning the same clip again keeps the name.
    pub fn assign(&mut self, key: Key, clip: &str, slot: Slot) -> Result<(), ShowError> {
        self.require_slot(slot)?;
        let clip = normalize_clip(clip)?;
        let name = self.kept_name(key, AssignmentTarget::Clip(clip.clone()));
        self.assignments.insert(
            key,
            Assignment {
                target: AssignmentTarget::Clip(clip),
                name,
                slot,
            },
        );
        Ok(())
    }

    /// Cuts this key to black. There is no clip file.
    ///
    /// Assigning black again keeps the name. Switching from a clip clears it.
    pub fn assign_black(&mut self, key: Key, slot: Slot) -> Result<(), ShowError> {
        self.require_slot(slot)?;
        let name = self.kept_name(key, AssignmentTarget::Black);
        self.assignments.insert(
            key,
            Assignment {
                target: AssignmentTarget::Black,
                name,
                slot,
            },
        );
        Ok(())
    }

    fn require_slot(&self, slot: Slot) -> Result<(), ShowError> {
        if self.template.slots().contains(&slot) {
            Ok(())
        } else {
            Err(ShowError::SlotNotInTemplate)
        }
    }

    /// Sets the short name drawn on an assigned key. An empty name clears it.
    ///
    /// A rejected name leaves the previous name in place.
    pub fn set_key_name(&mut self, key: Key, name: &str) -> Result<(), ShowError> {
        if !self.assignments.contains_key(&key) {
            return Err(ShowError::UnassignedKey);
        }
        let name = normalize_key_name(name)?;
        if let Some(assignment) = self.assignments.get_mut(&key) {
            assignment.name = name;
        }
        Ok(())
    }

    pub fn clear(&mut self, key: Key) {
        self.assignments.remove(&key);
    }

    fn kept_name(&self, key: Key, next: AssignmentTarget) -> Option<String> {
        let current = self.assignments.get(&key)?;
        (current.target == next)
            .then(|| current.name.clone())
            .flatten()
    }

    /// Bindings for the current display-mode session.
    ///
    /// When `locked` is true, clip keys do nothing. The lock chord and the exit
    /// chord are always present.
    pub fn playback_bindings(&self, locked: bool) -> Vec<PlaybackBinding> {
        let mut bindings = Vec::new();
        if !locked {
            for (key, assignment) in &self.assignments {
                let action = match &assignment.target {
                    AssignmentTarget::Clip(clip) => BindingAction::Play {
                        clip: clip.clone(),
                        slot: assignment.slot,
                    },
                    AssignmentTarget::Black => BindingAction::Black {
                        slot: assignment.slot,
                    },
                };
                bindings.push(PlaybackBinding {
                    chord: KeyChord::plain(*key),
                    action,
                });
            }
        }
        bindings.push(PlaybackBinding {
            chord: KeyChord::toggle_lock(),
            action: BindingAction::ToggleLock,
        });
        bindings.push(PlaybackBinding {
            chord: KeyChord::exit(),
            action: BindingAction::ExitDisplay,
        });
        bindings
    }
}

fn normalize_key_name(name: &str) -> Result<Option<String>, ShowError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.chars().count() > MAX_KEY_NAME_CHARS {
        return Err(ShowError::InvalidKeyName {
            reason: "use 24 characters or fewer",
        });
    }
    if trimmed.chars().any(char::is_control) {
        return Err(ShowError::InvalidKeyName {
            reason: "the name cannot contain a line break",
        });
    }
    Ok(Some(trimmed.to_string()))
}

fn require_directory(directory: PathBuf) -> Result<PathBuf, ShowError> {
    if directory.as_os_str().is_empty() {
        Err(ShowError::EmptyMediaDirectory)
    } else {
        Ok(directory)
    }
}

fn normalize_clip(path: &str) -> Result<String, ShowError> {
    if path.is_empty() {
        return Err(ShowError::InvalidClipPath {
            path: path.to_string(),
            reason: "the path is empty",
        });
    }
    if path.contains('\0') {
        return Err(ShowError::InvalidClipPath {
            path: path.to_string(),
            reason: "the path contains a null byte",
        });
    }
    let raw = Path::new(path);
    if raw.is_absolute() {
        return Err(ShowError::InvalidClipPath {
            path: path.to_string(),
            reason: "the path is absolute; clips stay inside the media directory",
        });
    }
    let mut parts = Vec::new();
    for component in raw.components() {
        match component {
            Component::Normal(part) => {
                let Some(part) = part.to_str() else {
                    return Err(ShowError::InvalidClipPath {
                        path: path.to_string(),
                        reason: "the path is not valid Unicode",
                    });
                };
                parts.push(part);
            }
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(ShowError::InvalidClipPath {
                    path: path.to_string(),
                    reason: "the path escapes the media directory",
                });
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(ShowError::InvalidClipPath {
                    path: path.to_string(),
                    reason: "the path is absolute; clips stay inside the media directory",
                });
            }
        }
    }
    if parts.is_empty() {
        return Err(ShowError::InvalidClipPath {
            path: path.to_string(),
            reason: "the path is empty",
        });
    }
    Ok(parts.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(name: &str) -> Key {
        Key::from_name(name).unwrap_or_else(|| panic!("key {name}"))
    }

    fn show() -> Show {
        Show::new("/shows/opening").unwrap()
    }

    #[test]
    fn assigns_and_replaces_a_clip() {
        let mut show = show();
        show.assign(key("a"), "intro.mp4", Slot::Full).unwrap();
        show.assign(key("b"), "loops/room.mp4", Slot::Full).unwrap();
        show.assign(key("a"), "intro-v2.mp4", Slot::Full).unwrap();
        assert_eq!(show.clip(key("a")), Some("intro-v2.mp4"));
        assert_eq!(show.clip(key("b")), Some("loops/room.mp4"));
        assert_eq!(show.assignments().count(), 2);
    }

    #[test]
    fn the_same_clip_can_sit_on_two_keys() {
        let mut show = show();
        show.assign(key("a"), "loop.mp4", Slot::Full).unwrap();
        show.assign(key("1"), "loop.mp4", Slot::Full).unwrap();
        assert_eq!(show.clip(key("a")), show.clip(key("1")));
    }

    #[test]
    fn clear_removes_one_key() {
        let mut show = show();
        show.assign(key("space"), "hold.mp4", Slot::Full).unwrap();
        show.clear(key("space"));
        show.clear(key("a"));
        assert_eq!(show.clip(key("space")), None);
    }

    #[test]
    fn rejected_clip_paths_leave_the_show_unchanged() {
        let mut show = show();
        show.assign(key("a"), "keep.mp4", Slot::Full).unwrap();
        let before = show.clone();
        for path in [
            "",
            ".",
            "..",
            "../secret.mp4",
            "/tmp/other.mp4",
            "foo/../../x.mp4",
        ] {
            assert!(show.assign(key("b"), path, Slot::Full).is_err(), "{path}");
        }
        assert_eq!(show, before);
    }

    #[test]
    fn dotted_clip_paths_normalize_inside_the_directory() {
        let mut show = show();
        show.assign(key("f2"), "room/./take.mp4", Slot::Full)
            .unwrap();
        assert_eq!(show.clip(key("f2")), Some("room/take.mp4"));
    }

    #[test]
    fn key_names_round_trip_and_reject_modifiers() {
        for name in ["a", "z", "0", "9", "f1", "f12", "space", "left", "up"] {
            let parsed = Key::from_name(name).unwrap();
            assert_eq!(parsed.name(), name);
        }
        assert_eq!(Key::from_name("F12").unwrap().name(), "f12");
        assert_eq!(Key::from_name("  K ").unwrap().label(), "K");
        assert!(Key::from_name("f13").is_none());
        assert!(Key::from_name("f01").is_none());
        assert!(Key::from_name("ctrl").is_none());
        assert!(Key::from_name("shift").is_none());
    }

    #[test]
    fn unlocked_bindings_play_and_always_keep_the_chords() {
        let mut show = show();
        show.assign(key("k"), "k.mp4", Slot::Full).unwrap();
        show.assign(key("a"), "a.mp4", Slot::Full).unwrap();
        let bindings = show.playback_bindings(false);
        assert!(bindings.iter().any(|binding| {
            binding.chord == KeyChord::plain(key("k"))
                && binding.action
                    == BindingAction::Play {
                        clip: "k.mp4".to_string(),
                        slot: Slot::Full,
                    }
        }));
        assert!(bindings.iter().any(|binding| {
            binding.chord == KeyChord::toggle_lock() && binding.action == BindingAction::ToggleLock
        }));
        assert!(bindings.iter().any(|binding| {
            binding.chord == KeyChord::exit() && binding.action == BindingAction::ExitDisplay
        }));
        assert_eq!(KeyChord::exit().label(), "Ctrl+Shift+K");
        assert_eq!(KeyChord::toggle_lock().label(), "Ctrl+Shift+L");
    }

    #[test]
    fn a_name_stays_with_the_same_assignment_and_a_bad_name_does_not() {
        let mut show = show();
        assert!(show.set_key_name(key("a"), "Doorway").is_err());
        show.assign(key("a"), "door.mp4", Slot::Full).unwrap();
        show.set_key_name(key("a"), "  Doorway  ").unwrap();
        assert_eq!(show.key_name(key("a")), Some("Doorway"));
        show.assign(key("a"), "door.mp4", Slot::Full).unwrap();
        assert_eq!(show.key_name(key("a")), Some("Doorway"));
        show.assign(key("a"), "other.mp4", Slot::Full).unwrap();
        assert_eq!(show.key_name(key("a")), None);
        show.set_key_name(key("a"), "Hold").unwrap();
        let named = show.clone();
        assert!(
            show.set_key_name(key("a"), "this name is far too long")
                .is_err()
        );
        assert!(show.set_key_name(key("a"), "two\nlines").is_err());
        show.set_key_name(key("a"), "   ").unwrap();
        assert_eq!(show.key_name(key("a")), None);
        assert_eq!(named.key_name(key("a")), Some("Hold"));
    }

    #[test]
    fn black_is_a_key_with_no_clip_and_lock_hides_it() {
        let mut show = show();
        show.assign(key("a"), "a.mp4", Slot::Full).unwrap();
        show.set_key_name(key("a"), "Clip").unwrap();
        show.assign_black(key("a"), Slot::Full).unwrap();
        assert!(show.is_black(key("a")));
        assert_eq!(show.clip(key("a")), None);
        assert_eq!(show.key_name(key("a")), None);
        show.set_key_name(key("a"), "Hold").unwrap();
        show.assign_black(key("a"), Slot::Full).unwrap();
        assert_eq!(show.key_name(key("a")), Some("Hold"));
        let bindings = show.playback_bindings(false);
        assert!(bindings.iter().any(|binding| {
            binding.chord == KeyChord::plain(key("a"))
                && binding.action == BindingAction::Black { slot: Slot::Full }
        }));
        let locked = show.playback_bindings(true);
        assert!(
            locked
                .iter()
                .all(|binding| !matches!(binding.action, BindingAction::Black { .. }))
        );
        assert_eq!(locked.len(), 2);
    }

    #[test]
    fn a_layout_keeps_keys_and_rejects_a_slot_it_does_not_have() {
        let mut show = show();
        show.set_template(Template::SideBySide);
        show.assign(key("a"), "left.mp4", Slot::Left).unwrap();
        show.assign(key("b"), "right.mp4", Slot::Right).unwrap();
        let before = show.clone();
        assert!(show.assign(key("c"), "nope.mp4", Slot::Top).is_err());
        assert_eq!(show, before);
        show.set_template(Template::Full);
        assert_eq!(show.assignment(key("a")).unwrap().slot(), Slot::Full);
        assert_eq!(show.assignment(key("b")).unwrap().slot(), Slot::Full);
        let play = show.playback_bindings(false);
        assert!(play.iter().any(|binding| {
            matches!(
                binding.action,
                BindingAction::Play {
                    slot: Slot::Full,
                    ..
                }
            )
        }));
    }

    #[test]
    fn auto_lock_rejects_zero_and_keeps_the_previous_value() {
        let mut show = show();
        assert!(show.auto_lock_seconds().is_none());
        show.set_auto_lock_seconds(Some(AUTO_LOCK_SECONDS)).unwrap();
        assert!(show.set_auto_lock_seconds(Some(0)).is_err());
        assert!(
            show.set_auto_lock_seconds(Some(MAX_AUTO_LOCK_SECONDS + 1))
                .is_err()
        );
        assert_eq!(show.auto_lock_seconds(), Some(AUTO_LOCK_SECONDS));
        show.set_auto_lock_seconds(None).unwrap();
        assert!(show.auto_lock_seconds().is_none());
    }

    #[test]
    fn locked_bindings_drop_clip_keys_and_keep_the_chords() {
        let mut show = show();
        show.assign(key("a"), "a.mp4", Slot::Full).unwrap();
        show.assign_black(key("z"), Slot::Full).unwrap();
        show.set_start_locked(true);
        let bindings = show.playback_bindings(true);
        assert!(bindings.iter().all(|binding| {
            !matches!(
                binding.action,
                BindingAction::Play { .. } | BindingAction::Black { .. }
            )
        }));
        assert_eq!(bindings.len(), 2);
        let still_playable = show.playback_bindings(false);
        assert!(
            still_playable
                .iter()
                .any(|binding| matches!(binding.action, BindingAction::Play { .. }))
        );
    }

    #[test]
    fn display_and_directory_edits_keep_the_previous_value_on_failure() {
        let mut show = show();
        let display = OutputDisplay::new("LG UltraFine", 1).unwrap();
        show.set_output_display(Some(display));
        assert!(OutputDisplay::new("", 2).is_err());
        assert_eq!(show.output_display().unwrap().name(), "LG UltraFine");
        assert_eq!(show.output_display().unwrap().index(), 1);
        assert!(show.set_media_directory("").is_err());
        assert_eq!(show.media_directory(), Path::new("/shows/opening"));
        show.set_muted(true);
        assert!(show.muted());
    }
}
