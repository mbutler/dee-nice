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
use std::path::{Path, PathBuf};

use dee_nice_core::{BindingAction, Key, KeyChord, PlaybackBinding};

use crate::media::{self, ResolveError};

pub const EVENT_SCRIPT: &str = r#"mp.register_script_message("dee-nice", function(action, detail)
    local seq = (mp.get_property_number("user-data/dee-nice/seq") or 0) + 1
    mp.set_property("user-data/dee-nice/seq", tostring(seq))
    local body = action
    if detail ~= nil and detail ~= "" then
        body = action .. " " .. detail
    end
    mp.set_property("user-data/dee-nice/event", body .. ":" .. tostring(seq))
end)
"#;

#[derive(Debug)]
pub enum PlanError {
    Clip(ResolveError),
    DisplayName,
}

impl std::fmt::Display for PlanError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Clip(error) => write!(formatter, "{error}"),
            Self::DisplayName => write!(formatter, "the display name cannot be passed to mpv"),
        }
    }
}

pub struct LaunchPlan {
    pub arguments: Vec<String>,
    pub input_conf: String,
    /// Play bindings only. mpv will not drop keys that were loaded from a file,
    /// so these live in a section the session can disable while locked.
    pub clip_section: String,
    pub initial_clip: Option<String>,
}

pub struct LaunchSpec<'a> {
    pub media_directory: &'a Path,
    pub bindings: &'a [PlaybackBinding],
    pub display_name: &'a str,
    pub muted: bool,
    pub initial_clip: Option<&'a str>,
    /// Relative clip path to a finished lighter copy. Missing keys play the source.
    pub ready: &'a BTreeMap<String, PathBuf>,
    /// Full layout loads the file in the key binding. A divided layout only
    /// reports the key, and the session plays it in that slot.
    pub direct: bool,
    pub input_conf: &'a Path,
    pub ipc_socket: &'a Path,
    pub script: &'a Path,
}

pub fn launch_plan(spec: &LaunchSpec<'_>) -> Result<LaunchPlan, PlanError> {
    if spec.display_name.is_empty()
        || spec.display_name.contains(['\n', '\r'])
        || spec.display_name.starts_with('-')
    {
        return Err(PlanError::DisplayName);
    }
    let input_conf = chord_input_conf();
    let clip_section = clip_section(spec.media_directory, spec.bindings, spec.ready, spec.direct)?;
    let initial_clip = match spec.initial_clip {
        Some(clip) => Some(mpv_path(&playback_path(
            spec.media_directory,
            clip,
            spec.ready,
        )?)?),
        None => None,
    };
    Ok(LaunchPlan {
        arguments: mpv_arguments(spec),
        input_conf,
        clip_section,
        initial_clip,
    })
}

/// Lock and exit stay in the file so they work before IPC is connected.
pub fn chord_input_conf() -> String {
    format!(
        "# dee-nice display mode\n{} script-message dee-nice lock\n{} quit\n",
        mpv_key(KeyChord::toggle_lock()),
        mpv_key(KeyChord::exit()),
    )
}

pub fn clip_section(
    directory: &Path,
    bindings: &[PlaybackBinding],
    ready: &BTreeMap<String, PathBuf>,
    direct: bool,
) -> Result<String, PlanError> {
    let mut lines = Vec::new();
    for binding in bindings {
        let line = match &binding.action {
            BindingAction::Play { clip, .. } if direct => {
                let path = playback_path(directory, clip, ready)?;
                format!(
                    "{} loadfile {} replace; script-message dee-nice play {}",
                    mpv_key(binding.chord),
                    quote_mpv(&mpv_path(&path)?)?,
                    binding.chord.key().name(),
                )
            }
            BindingAction::Black { .. } if direct => format!(
                "{} stop; script-message dee-nice play {}",
                mpv_key(binding.chord),
                binding.chord.key().name(),
            ),
            BindingAction::Play { .. } | BindingAction::Black { .. } => format!(
                "{} script-message dee-nice play {}",
                mpv_key(binding.chord),
                binding.chord.key().name(),
            ),
            BindingAction::ToggleLock | BindingAction::ExitDisplay => continue,
        };
        lines.push(line);
    }
    Ok(lines.join("\n"))
}

pub fn loadfile_command(
    directory: &Path,
    clip: &str,
    ready: &BTreeMap<String, PathBuf>,
) -> Result<serde_json::Value, String> {
    let path = playback_path(directory, clip, ready).map_err(|error| error.to_string())?;
    let path = mpv_path(&path).map_err(|error| error.to_string())?;
    Ok(serde_json::json!({
        "command": ["loadfile", path, "replace"]
    }))
}

fn playback_path(
    directory: &Path,
    clip: &str,
    ready: &BTreeMap<String, PathBuf>,
) -> Result<PathBuf, PlanError> {
    if let Some(derivative) = ready.get(clip)
        && derivative.is_file()
    {
        return Ok(derivative.clone());
    }
    media::resolve_clip(directory, clip).map_err(PlanError::Clip)
}

#[cfg(test)]
pub fn render_input_conf(
    directory: &Path,
    bindings: &[PlaybackBinding],
) -> Result<String, PlanError> {
    let mut lines = vec!["# dee-nice display mode".to_string()];
    for binding in bindings {
        let key = mpv_key(binding.chord);
        let command = match &binding.action {
            BindingAction::Play { clip, .. } => {
                let path = media::resolve_clip(directory, clip).map_err(PlanError::Clip)?;
                format!("loadfile {} replace", quote_mpv(&mpv_path(&path)?)?)
            }
            BindingAction::Black { .. } => "stop".to_string(),
            BindingAction::ToggleLock => "script-message dee-nice lock".to_string(),
            BindingAction::ExitDisplay => "quit".to_string(),
        };
        lines.push(format!("{key} {command}"));
    }
    lines.push(String::new());
    Ok(lines.join("\n"))
}

fn mpv_arguments(spec: &LaunchSpec<'_>) -> Vec<String> {
    let mut arguments = shared_arguments(spec.display_name, spec.muted);
    arguments.extend([
        "--fullscreen".to_string(),
        format!("--fs-screen-name={}", spec.display_name),
        "--ontop=yes".to_string(),
        format!("--input-conf={}", spec.input_conf.display()),
        format!("--input-ipc-server={}", spec.ipc_socket.display()),
        format!("--script={}", spec.script.display()),
    ]);
    if !spec.direct {
        // Stay in the current Space so the slot windows can sit on top.
        arguments.push("--native-fs=no".to_string());
        arguments.push("--ontop-level=0".to_string());
    }
    arguments
}

pub fn slot_arguments(
    display_name: &str,
    geometry: &str,
    muted: bool,
    ipc_socket: &Path,
    input_conf: &Path,
    script: &Path,
) -> Vec<String> {
    let mut arguments = shared_arguments(display_name, muted);
    arguments.extend([
        "--no-border".to_string(),
        "--ontop=yes".to_string(),
        "--ontop-level=1".to_string(),
        "--focus-on=never".to_string(),
        "--macos-geometry-calculation=whole".to_string(),
        format!("--geometry={geometry}"),
        format!("--input-conf={}", input_conf.display()),
        format!("--input-ipc-server={}", ipc_socket.display()),
        format!("--script={}", script.display()),
    ]);
    arguments
}

fn shared_arguments(display_name: &str, muted: bool) -> Vec<String> {
    let mut arguments = vec![
        "--no-config".to_string(),
        "--idle=yes".to_string(),
        "--force-window=immediate".to_string(),
        format!("--screen-name={display_name}"),
        "--loop-file=inf".to_string(),
        "--keepaspect=yes".to_string(),
        "--no-osc".to_string(),
        "--osd-level=0".to_string(),
        "--no-input-default-bindings".to_string(),
        "--cursor-autohide=1000".to_string(),
        "--msg-level=all=error".to_string(),
    ];
    if muted {
        arguments.push("--mute=yes".to_string());
    }
    arguments
}

/// mpv key name for a chord.
///
/// mpv treats Shift plus a letter as the uppercase letter, so Ctrl+Shift+K is
/// `ctrl+K`. A plain K key stays `k`.
pub fn mpv_key(chord: KeyChord) -> String {
    let mut parts = Vec::new();
    if chord.ctrl() {
        parts.push("ctrl".to_string());
    }
    if chord.alt() {
        parts.push("alt".to_string());
    }
    let base = mpv_base(chord.key());
    let key = if chord.shift() && is_ascii_letter(&base) {
        base.to_ascii_uppercase()
    } else {
        if chord.shift() {
            parts.push("shift".to_string());
        }
        base
    };
    parts.push(key);
    parts.join("+")
}

fn is_ascii_letter(name: &str) -> bool {
    name.len() == 1 && name.chars().all(|ch| ch.is_ascii_lowercase())
}

fn mpv_base(key: Key) -> String {
    match key.name().as_str() {
        "space" => "SPACE".to_string(),
        "left" => "LEFT".to_string(),
        "right" => "RIGHT".to_string(),
        "up" => "UP".to_string(),
        "down" => "DOWN".to_string(),
        name if name.strip_prefix('f').is_some_and(|rest| {
            rest.parse::<u8>()
                .ok()
                .is_some_and(|number| (1..=12).contains(&number) && name == format!("f{number}"))
        }) =>
        {
            format!("F{}", &name[1..])
        }
        name => name.to_string(),
    }
}

pub fn quote_mpv(text: &str) -> Result<String, PlanError> {
    if text.contains(['\n', '\r']) {
        return Err(PlanError::Clip(ResolveError::NotUnicode {
            clip: text.to_string(),
        }));
    }
    let mut quoted = String::from("\"");
    for ch in text.chars() {
        if ch == '\\' || ch == '"' {
            quoted.push('\\');
        }
        quoted.push(ch);
    }
    quoted.push('"');
    Ok(quoted)
}

fn mpv_path(path: &Path) -> Result<String, PlanError> {
    path.to_str().map(str::to_string).ok_or_else(|| {
        PlanError::Clip(ResolveError::NotUnicode {
            clip: path.display().to_string(),
        })
    })
}

pub fn mpv_binary() -> Option<PathBuf> {
    crate::tools::find_tool("DEE_NICE_MPV", mpv_file_name())
}

fn mpv_file_name() -> &'static str {
    if cfg!(windows) { "mpv.exe" } else { "mpv" }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dee_nice_core::{Key, Show, Slot};
    use std::fs;
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
            let path = std::env::temp_dir().join(format!("dee-nice-plan-{tick}-{n}"));
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
    fn chords_use_mpv_letter_normalization() {
        assert_eq!(mpv_key(KeyChord::plain(key("k"))), "k");
        assert_eq!(mpv_key(KeyChord::plain(key("space"))), "SPACE");
        assert_eq!(mpv_key(KeyChord::plain(key("f12"))), "F12");
        assert_eq!(mpv_key(KeyChord::plain(key("left"))), "LEFT");
        assert_eq!(mpv_key(KeyChord::exit()), "ctrl+K");
        assert_eq!(mpv_key(KeyChord::toggle_lock()), "ctrl+L");
    }

    #[test]
    fn quotes_spaces_and_quotes() {
        assert_eq!(
            quote_mpv(r#"/shows/my clip "a".mp4"#).unwrap(),
            r#""/shows/my clip \"a\".mp4""#
        );
    }

    #[test]
    fn locked_conf_has_no_clips_and_unlocked_conf_loads_the_file() {
        let dir = TempDir::new();
        fs::write(dir.0.join("a.mp4"), b"video").unwrap();
        let mut show = Show::new(&dir.0).unwrap();
        show.assign(key("a"), "a.mp4", Slot::Full).unwrap();
        let open = render_input_conf(&dir.0, &show.playback_bindings(false)).unwrap();
        assert!(open.contains("loadfile "));
        assert!(open.contains(" replace"));
        assert!(open.contains("script-message dee-nice lock"));
        assert!(open.contains("ctrl+K quit"));
        let locked = render_input_conf(&dir.0, &show.playback_bindings(true)).unwrap();
        assert!(!locked.contains("loadfile"));
        assert!(locked.contains("ctrl+L script-message dee-nice lock"));
        assert!(locked.contains("ctrl+K quit"));
        let section = clip_section(
            &dir.0,
            &show.playback_bindings(false),
            &BTreeMap::new(),
            true,
        )
        .unwrap();
        assert!(section.contains("loadfile "));
        assert!(section.contains("script-message dee-nice play a"));
        assert!(!section.contains("quit"));
        show.assign_black(key("b"), Slot::Full).unwrap();
        let with_black = clip_section(
            &dir.0,
            &show.playback_bindings(false),
            &BTreeMap::new(),
            true,
        )
        .unwrap();
        assert!(with_black.contains("b stop; script-message dee-nice play b"));
        let routed = clip_section(
            &dir.0,
            &show.playback_bindings(false),
            &BTreeMap::new(),
            false,
        )
        .unwrap();
        assert!(!routed.contains("loadfile"));
        assert!(routed.contains("script-message dee-nice play a"));
        assert!(routed.contains("b script-message dee-nice play b"));
        let locked_black = clip_section(
            &dir.0,
            &show.playback_bindings(true),
            &BTreeMap::new(),
            true,
        )
        .unwrap();
        assert!(locked_black.is_empty());
        let chords = chord_input_conf();
        assert!(!chords.contains("loadfile"));
        assert!(chords.contains("ctrl+L script-message dee-nice lock"));
        assert!(chords.contains("ctrl+K quit"));
    }

    #[test]
    fn launch_arguments_name_the_display_and_mute_when_asked() {
        let dir = TempDir::new();
        fs::write(dir.0.join("a.mp4"), b"video").unwrap();
        let input = dir.0.join("input.conf");
        let ipc = Path::new("/tmp/dee-nice-test.sock");
        let script = dir.0.join("events.lua");
        let ready = BTreeMap::new();
        let spec = LaunchSpec {
            media_directory: &dir.0,
            bindings: &[],
            display_name: "Studio Display",
            muted: true,
            initial_clip: Some("a.mp4"),
            ready: &ready,
            direct: true,
            input_conf: &input,
            ipc_socket: ipc,
            script: &script,
        };
        let plan = launch_plan(&spec).unwrap();
        assert!(plan.arguments.iter().any(|arg| arg == "--no-config"));
        assert!(
            plan.arguments
                .iter()
                .any(|arg| arg == "--screen-name=Studio Display")
        );
        assert!(
            plan.arguments
                .iter()
                .any(|arg| arg == "--fs-screen-name=Studio Display")
        );
        assert!(plan.arguments.iter().any(|arg| arg == "--mute=yes"));
        assert!(plan.arguments.iter().any(|arg| arg == "--loop-file=inf"));
        let clip = plan.initial_clip.unwrap();
        assert!(clip.ends_with("a.mp4"));
        let audible = LaunchSpec {
            muted: false,
            initial_clip: None,
            ..spec
        };
        let audible = launch_plan(&audible).unwrap();
        assert!(!audible.arguments.iter().any(|arg| arg.contains("mute")));
        assert!(audible.initial_clip.is_none());
    }
}
