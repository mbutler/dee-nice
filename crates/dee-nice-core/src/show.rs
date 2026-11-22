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
    Play { clip: String },
    ToggleLock,
    ExitDisplay,
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

/// The durable show: a media folder, one clip per key, and playback settings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Show {
    pub(crate) media_directory: PathBuf,
    pub(crate) assignments: BTreeMap<Key, String>,
    pub(crate) muted: bool,
    pub(crate) start_locked: bool,
    pub(crate) output_display: Option<OutputDisplay>,
}

impl Show {
    pub fn new(media_directory: impl Into<PathBuf>) -> Result<Self, ShowError> {
        Ok(Self {
            media_directory: require_directory(media_directory.into())?,
            assignments: BTreeMap::new(),
            muted: false,
            start_locked: false,
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

    pub fn output_display(&self) -> Option<&OutputDisplay> {
        self.output_display.as_ref()
    }

    pub fn set_output_display(&mut self, display: Option<OutputDisplay>) {
        self.output_display = display;
    }

    pub fn clip(&self, key: Key) -> Option<&str> {
        self.assignments.get(&key).map(String::as_str)
    }

    pub fn assignments(&self) -> impl Iterator<Item = (Key, &str)> + '_ {
        self.assignments
            .iter()
            .map(|(key, clip)| (*key, clip.as_str()))
    }

    /// Assigns a clip inside the media directory. A new assignment replaces the old one.
    ///
    /// The clip path is relative. Absolute paths and `..` are rejected, and a
    /// rejected assignment leaves the show unchanged.
    pub fn assign(&mut self, key: Key, clip: &str) -> Result<(), ShowError> {
        let clip = normalize_clip(clip)?;
        self.assignments.insert(key, clip);
        Ok(())
    }

    pub fn clear(&mut self, key: Key) {
        self.assignments.remove(&key);
    }

    /// Bindings for the current display-mode session.
    ///
    /// When `locked` is true, clip keys do nothing. The lock chord and the exit
    /// chord are always present.
    pub fn playback_bindings(&self, locked: bool) -> Vec<PlaybackBinding> {
        let mut bindings = Vec::new();
        if !locked {
            for (key, clip) in &self.assignments {
                bindings.push(PlaybackBinding {
                    chord: KeyChord::plain(*key),
                    action: BindingAction::Play { clip: clip.clone() },
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
        show.assign(key("a"), "intro.mp4").unwrap();
        show.assign(key("b"), "loops/room.mp4").unwrap();
        show.assign(key("a"), "intro-v2.mp4").unwrap();
        assert_eq!(show.clip(key("a")), Some("intro-v2.mp4"));
        assert_eq!(show.clip(key("b")), Some("loops/room.mp4"));
        assert_eq!(show.assignments().count(), 2);
    }

    #[test]
    fn the_same_clip_can_sit_on_two_keys() {
        let mut show = show();
        show.assign(key("a"), "loop.mp4").unwrap();
        show.assign(key("1"), "loop.mp4").unwrap();
        assert_eq!(show.clip(key("a")), show.clip(key("1")));
    }

    #[test]
    fn clear_removes_one_key() {
        let mut show = show();
        show.assign(key("space"), "hold.mp4").unwrap();
        show.clear(key("space"));
        show.clear(key("a"));
        assert_eq!(show.clip(key("space")), None);
    }

    #[test]
    fn rejected_clip_paths_leave_the_show_unchanged() {
        let mut show = show();
        show.assign(key("a"), "keep.mp4").unwrap();
        let before = show.clone();
        for path in [
            "",
            ".",
            "..",
            "../secret.mp4",
            "/tmp/other.mp4",
            "foo/../../x.mp4",
        ] {
            assert!(show.assign(key("b"), path).is_err(), "{path}");
        }
        assert_eq!(show, before);
    }

    #[test]
    fn dotted_clip_paths_normalize_inside_the_directory() {
        let mut show = show();
        show.assign(key("f2"), "room/./take.mp4").unwrap();
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
        show.assign(key("k"), "k.mp4").unwrap();
        show.assign(key("a"), "a.mp4").unwrap();
        let bindings = show.playback_bindings(false);
        assert!(bindings.iter().any(|binding| {
            binding.chord == KeyChord::plain(key("k"))
                && binding.action
                    == BindingAction::Play {
                        clip: "k.mp4".to_string(),
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
    fn locked_bindings_drop_clip_keys_and_keep_the_chords() {
        let mut show = show();
        show.assign(key("a"), "a.mp4").unwrap();
        show.set_start_locked(true);
        let bindings = show.playback_bindings(true);
        assert!(
            bindings
                .iter()
                .all(|binding| !matches!(binding.action, BindingAction::Play { .. }))
        );
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
