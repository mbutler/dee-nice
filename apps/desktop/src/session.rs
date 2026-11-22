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
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use dee_nice_core::{BindingAction, PlaybackBinding, Show, Slot};

use crate::layout;
use crate::playback::{self, EVENT_SCRIPT};
use crate::readiness::opening_cues;

pub enum SessionUpdate {
    Running,
    Stopped(String),
}

pub struct MpvSession {
    child: Child,
    stderr: Arc<Mutex<String>>,
    stderr_thread: Option<JoinHandle<()>>,
    ipc: Option<std::os::unix::net::UnixStream>,
    ipc_buf: Vec<u8>,
    ipc_path: PathBuf,
    directory: PathBuf,
    media_directory: PathBuf,
    bindings: Vec<PlaybackBinding>,
    ready: BTreeMap<String, PathBuf>,
    locked: bool,
    clip_section: String,
    clips_installed: bool,
    initial_clip: Option<String>,
    live_slots: BTreeMap<Slot, String>,
    routed: bool,
    slots: Vec<SlotPlayer>,
    auto_lock: Option<Duration>,
    idle_since: Instant,
    stop_reason: Option<String>,
    observed: bool,
    awake: Option<Child>,
}

impl MpvSession {
    pub fn start(
        show: &Show,
        ready: &BTreeMap<String, PathBuf>,
        screen: (u32, u32),
    ) -> Result<Self, String> {
        let binary = playback::mpv_binary().ok_or_else(|| "mpv was not found.".to_string())?;
        let display_name = show
            .output_display()
            .map(|display| display.name().to_string())
            .ok_or_else(|| "Choose the display that should go fullscreen.".to_string())?;
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let directory =
            std::env::temp_dir().join(format!("dee-nice-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
        match Self::spawn(show, ready, &binary, &display_name, &directory, screen) {
            Ok(session) => Ok(session),
            Err(error) => {
                let _ = fs::remove_dir_all(&directory);
                Err(error)
            }
        }
    }

    fn spawn(
        show: &Show,
        ready: &BTreeMap<String, PathBuf>,
        binary: &Path,
        display_name: &str,
        directory: &Path,
        screen: (u32, u32),
    ) -> Result<Self, String> {
        let input_conf_path = directory.join("input.conf");
        let script_path = directory.join("events.lua");
        // Socket paths on macOS have to stay short, so the socket stays in the
        // system temp directory instead of the longer session folder.
        let ipc_path = short_socket_path(0);
        let _ = fs::remove_file(&ipc_path);
        fs::write(&script_path, EVENT_SCRIPT).map_err(|error| error.to_string())?;
        let locked = show.start_locked();
        // Clip keys are installed from the unlocked map. Lock only disables that
        // section; it does not forget the assignments.
        let bindings = show.playback_bindings(false);
        let cues = opening_cues(show);
        let direct = show.template().is_full();
        let initial = if direct {
            cues.first().and_then(|cue| cue.clip.clone())
        } else {
            None
        };
        let plan = playback::launch_plan(&playback::LaunchSpec {
            media_directory: show.media_directory(),
            bindings: &bindings,
            display_name,
            muted: show.muted(),
            initial_clip: initial.as_deref(),
            ready,
            direct,
            input_conf: &input_conf_path,
            ipc_socket: &ipc_path,
            script: &script_path,
        })
        .map_err(|error| error.to_string())?;
        fs::write(&input_conf_path, &plan.input_conf).map_err(|error| error.to_string())?;
        let mut command = Command::new(binary);
        command
            .args(&plan.arguments)
            .current_dir(show.media_directory())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().map_err(|error| {
            let _ = fs::remove_dir_all(directory);
            format!("could not start mpv: {error}")
        })?;
        let stderr = child.stderr.take();
        let stdout = child.stdout.take();
        let log = Arc::new(Mutex::new(String::new()));
        let stderr_thread = stderr.map(|pipe| spawn_output_reader(pipe, Arc::clone(&log)));
        let stdout_thread = stdout.map(|pipe| spawn_output_reader(pipe, Arc::clone(&log)));
        let stderr_thread = match (stderr_thread, stdout_thread) {
            (Some(stderr_thread), Some(stdout_thread)) => {
                Some(join_both(stderr_thread, stdout_thread))
            }
            (Some(thread), None) | (None, Some(thread)) => Some(thread),
            (None, None) => None,
        };
        let mut slots = Vec::new();
        if !direct {
            let aspect = if screen.1 == 0 {
                16.0 / 9.0
            } else {
                f64::from(screen.0) / f64::from(screen.1)
            };
            for (index, slot) in show.template().slots().iter().copied().enumerate() {
                let geometry = layout::slot_frame(show.template(), slot, aspect).mpv_geometry();
                let pending = cues.iter().find(|cue| cue.slot == slot).and_then(|cue| {
                    cue.clip.as_deref().and_then(|clip| {
                        playback::loadfile_command(show.media_directory(), clip, ready).ok()
                    })
                });
                match spawn_slot(&SlotLaunch {
                    binary,
                    display_name,
                    geometry: &geometry,
                    muted: show.muted(),
                    directory: show.media_directory(),
                    input_conf: &input_conf_path,
                    script: &script_path,
                    index: u32::try_from(index + 1).unwrap_or(1),
                    slot,
                    pending,
                }) {
                    Ok(player) => slots.push(player),
                    Err(error) => {
                        let _ = child.kill();
                        for player in &mut slots {
                            let _ = player.child.kill();
                            let _ = fs::remove_file(&player.ipc_path);
                        }
                        return Err(error);
                    }
                }
            }
        }
        let mut live_slots = BTreeMap::new();
        for cue in &cues {
            live_slots.insert(cue.slot, cue.key_name.clone());
        }
        let awake = keep_awake(child.id());
        Ok(Self {
            child,
            stderr: log,
            stderr_thread,
            ipc: None,
            ipc_buf: Vec::new(),
            ipc_path,
            directory: directory.to_path_buf(),
            media_directory: show.media_directory().to_path_buf(),
            bindings,
            ready: ready.clone(),
            locked,
            clip_section: plan.clip_section,
            clips_installed: false,
            initial_clip: plan.initial_clip,
            live_slots,
            routed: !direct,
            slots,
            auto_lock: show
                .auto_lock_seconds()
                .map(|seconds| Duration::from_secs(u64::from(seconds))),
            idle_since: Instant::now(),
            stop_reason: None,
            observed: false,
            awake,
        })
    }

    pub fn poll(&mut self) -> SessionUpdate {
        if let Some(reason) = self.stop_reason.clone() {
            self.kill_all();
            return SessionUpdate::Stopped(reason);
        }
        match self.child.try_wait() {
            Ok(Some(status)) => {
                let stderr = self.stderr_text();
                self.stop_awake();
                self.kill_slots();
                SessionUpdate::Stopped(stop_message(status, &stderr))
            }
            Ok(None) => {
                if self.slot_closed() {
                    self.kill_all();
                    return SessionUpdate::Stopped("A slot window closed.".to_string());
                }
                self.pump_ipc();
                self.pump_slots();
                self.maybe_auto_lock();
                if let Some(reason) = &self.stop_reason {
                    SessionUpdate::Stopped(reason.clone())
                } else {
                    SessionUpdate::Running
                }
            }
            Err(error) => SessionUpdate::Stopped(format!("lost track of mpv: {error}")),
        }
    }

    fn pump_ipc(&mut self) {
        if self.ipc.is_none() {
            match std::os::unix::net::UnixStream::connect(&self.ipc_path) {
                Ok(stream) => {
                    let _ = stream.set_nonblocking(true);
                    self.ipc = Some(stream);
                }
                Err(_) => return,
            }
        }
        self.read_ipc();
        if self.ipc.is_some() && !self.clips_installed {
            self.install_clip_keys();
        }
        self.handle_lines();
        if self.ipc.is_none() {
            return;
        }
        if !self.observed {
            self.send_json(&serde_json::json!({
                "command": ["observe_property", 1, "user-data/dee-nice/event"]
            }));
            self.observed = true;
        }
        if let Some(clip) = self.initial_clip.clone() {
            self.send_json(&serde_json::json!({
                "command": ["loadfile", clip, "replace"]
            }));
            self.initial_clip = None;
        }
    }

    fn read_ipc(&mut self) {
        let Some(mut stream) = self.ipc.take() else {
            return;
        };
        let mut chunk = [0_u8; 4096];
        let mut keep = true;
        loop {
            match stream.read(&mut chunk) {
                Ok(0) => {
                    keep = false;
                    self.observed = false;
                    break;
                }
                Ok(count) => self.ipc_buf.extend_from_slice(&chunk[..count]),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => {
                    keep = false;
                    self.observed = false;
                    break;
                }
            }
            if self.ipc_buf.len() > 1_048_576 {
                self.ipc_buf.clear();
                break;
            }
        }
        if keep {
            self.ipc = Some(stream);
        }
    }

    fn handle_lines(&mut self) {
        while let Some(split) = self.ipc_buf.iter().position(|byte| *byte == b'\n') {
            let line: Vec<u8> = self.ipc_buf.drain(..=split).collect();
            let line = String::from_utf8_lossy(&line);
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };
            let is_event =
                value.get("event").and_then(|event| event.as_str()) == Some("property-change");
            if !is_event {
                continue;
            }
            if value.get("name").and_then(|name| name.as_str()) != Some("user-data/dee-nice/event")
            {
                continue;
            }
            let Some(data) = value.get("data").and_then(|data| data.as_str()) else {
                continue;
            };
            if let Some(event) = classify_event(data) {
                self.apply_event(event);
            }
        }
    }

    pub fn is_live(&self, name: &str) -> bool {
        self.live_slots.values().any(|key| key == name)
    }

    pub fn live_names(&self) -> Vec<&str> {
        self.live_slots.values().map(String::as_str).collect()
    }

    pub fn is_locked(&self) -> bool {
        self.locked
    }

    pub fn quit(&mut self) {
        if self.ipc.is_some() {
            self.send_json(&serde_json::json!({
                "command": ["quit"]
            }));
            return;
        }
        let _ = self.child.kill();
        self.kill_slots();
        self.stop_reason = Some("Left display mode.".to_string());
    }

    /// Point clip keys at lighter copies that finished after display mode started.
    ///
    /// The clip already on screen is left alone. The next press uses the new file.
    pub fn replace_ready(&mut self, ready: &BTreeMap<String, PathBuf>) {
        if &self.ready == ready {
            return;
        }
        self.ready = ready.clone();
        self.reinstall_section();
    }

    pub fn replace_bindings(&mut self, show: &Show) {
        self.bindings = show.playback_bindings(false);
        self.reinstall_section();
    }

    pub fn set_auto_lock(&mut self, seconds: Option<u32>) {
        self.auto_lock = seconds
            .filter(|seconds| *seconds > 0)
            .map(|seconds| Duration::from_secs(u64::from(seconds)));
        self.note_activity();
    }

    fn reinstall_section(&mut self) {
        let Ok(section) = playback::clip_section(
            &self.media_directory,
            &self.bindings,
            &self.ready,
            !self.routed,
        ) else {
            return;
        };
        if section == self.clip_section {
            return;
        }
        self.clip_section = section;
        if self.clips_installed && !self.clip_section.is_empty() {
            self.send_json(&serde_json::json!({
                "command": ["define-section", "dee-nice-clips", self.clip_section, "force"]
            }));
            if !self.locked {
                self.send_json(&serde_json::json!({
                    "command": ["enable-section", "dee-nice-clips"]
                }));
            }
        }
        for player in &mut self.slots {
            player.clips_installed = false;
        }
    }

    fn install_clip_keys(&mut self) {
        if self.clip_section.is_empty() {
            self.clips_installed = true;
            return;
        }
        // `force` is required. Without it, mpv accepts the section and then
        // ignores its keys.
        self.send_json(&serde_json::json!({
            "command": ["define-section", "dee-nice-clips", self.clip_section, "force"]
        }));
        if !self.locked {
            self.send_json(&serde_json::json!({
                "command": ["enable-section", "dee-nice-clips"]
            }));
        }
        self.clips_installed = true;
    }

    pub fn toggle_lock(&mut self) {
        self.note_activity();
        self.locked = !self.locked;
        self.apply_lock_section();
    }

    fn lock_keys(&mut self) {
        if self.locked {
            return;
        }
        self.locked = true;
        self.apply_lock_section();
    }

    fn apply_lock_section(&mut self) {
        if !self.clips_installed || self.clip_section.is_empty() {
            return;
        }
        let command = if self.locked {
            "disable-section"
        } else {
            "enable-section"
        };
        let value = serde_json::json!({
            "command": [command, "dee-nice-clips"]
        });
        self.send_json(&value);
        for player in &mut self.slots {
            player.send(&value);
        }
    }

    fn note_activity(&mut self) {
        self.idle_since = Instant::now();
    }

    fn maybe_auto_lock(&mut self) {
        if !auto_lock_due(self.locked, self.auto_lock, self.idle_since.elapsed()) {
            return;
        }
        self.lock_keys();
        self.note_activity();
    }

    fn apply_event(&mut self, event: ShowEvent) {
        match event {
            ShowEvent::Lock => self.toggle_lock(),
            ShowEvent::Play(key) => self.play_key(key),
        }
    }

    fn play_key(&mut self, key_name: String) {
        self.note_activity();
        let action = self
            .bindings
            .iter()
            .find(|binding| binding.chord.key().name() == key_name)
            .map(|binding| binding.action.clone());
        if let Some(action) = action {
            if let Some(slot) = action_slot(&action) {
                self.live_slots.insert(slot, key_name);
            }
            if self.routed {
                self.route(action);
            }
        }
    }

    fn route(&mut self, action: BindingAction) {
        let (slot, command) = match action {
            BindingAction::Play { clip, slot } => {
                let Ok(command) =
                    playback::loadfile_command(&self.media_directory, &clip, &self.ready)
                else {
                    return;
                };
                (slot, command)
            }
            BindingAction::Black { slot } => (
                slot,
                serde_json::json!({
                    "command": ["stop"]
                }),
            ),
            BindingAction::ToggleLock | BindingAction::ExitDisplay => return,
        };
        if let Some(player) = self.slots.iter_mut().find(|player| player.slot == slot) {
            player.enqueue(command);
        }
    }

    fn pump_slots(&mut self) {
        let section = self.clip_section.clone();
        let locked = self.locked;
        let mut events = Vec::new();
        for player in &mut self.slots {
            player.pump(&section, locked, &mut events);
        }
        for event in events {
            self.apply_event(event);
        }
    }

    fn slot_closed(&mut self) -> bool {
        self.slots
            .iter_mut()
            .any(|player| player.child.try_wait().ok().flatten().is_some())
    }

    fn kill_slots(&mut self) {
        for player in &mut self.slots {
            if player.child.try_wait().ok().flatten().is_none() {
                let _ = player.child.kill();
            }
            let _ = fs::remove_file(&player.ipc_path);
        }
    }

    fn kill_all(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
        }
        self.stop_awake();
        self.kill_slots();
    }

    fn stop_awake(&mut self) {
        if let Some(child) = self.awake.as_mut()
            && child.try_wait().ok().flatten().is_none()
        {
            let _ = child.kill();
        }
    }

    fn send_json(&mut self, value: &serde_json::Value) {
        let Some(stream) = self.ipc.as_mut() else {
            return;
        };
        let Ok(text) = serde_json::to_string(value) else {
            return;
        };
        let _ = stream.write_all(text.as_bytes());
        let _ = stream.write_all(b"\n");
        let _ = stream.flush();
    }

    fn stderr_text(&self) -> String {
        self.stderr
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }
}

impl Drop for MpvSession {
    fn drop(&mut self) {
        self.kill_all();
        let _ = self.child.wait();
        if let Some(child) = self.awake.as_mut() {
            let _ = child.wait();
        }
        for player in &mut self.slots {
            let _ = player.child.wait();
        }
        if let Some(handle) = self.stderr_thread.take() {
            let _ = handle.join();
        }
        let _ = fs::remove_file(&self.ipc_path);
        let _ = fs::remove_dir_all(&self.directory);
    }
}

fn short_socket_path(index: u32) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.subsec_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("dn-{}-{nonce}-{index}.sock", std::process::id()))
}

struct SlotPlayer {
    slot: Slot,
    child: Child,
    ipc: Option<std::os::unix::net::UnixStream>,
    ipc_buf: Vec<u8>,
    ipc_path: PathBuf,
    pending: Vec<serde_json::Value>,
    clips_installed: bool,
}

impl SlotPlayer {
    fn pump(&mut self, section: &str, locked: bool, events: &mut Vec<ShowEvent>) {
        if self.ipc.is_none() {
            match std::os::unix::net::UnixStream::connect(&self.ipc_path) {
                Ok(stream) => {
                    let _ = stream.set_nonblocking(true);
                    self.ipc = Some(stream);
                }
                Err(_) => return,
            }
        }
        self.read(events);
        if self.ipc.is_some() && !self.clips_installed && !section.is_empty() {
            self.send(&serde_json::json!({
                "command": ["define-section", "dee-nice-clips", section, "force"]
            }));
            if !locked {
                self.send(&serde_json::json!({
                    "command": ["enable-section", "dee-nice-clips"]
                }));
            }
            self.send(&serde_json::json!({
                "command": ["observe_property", 1, "user-data/dee-nice/event"]
            }));
            self.clips_installed = true;
        }
        self.flush();
    }

    fn read(&mut self, events: &mut Vec<ShowEvent>) {
        let Some(mut stream) = self.ipc.take() else {
            return;
        };
        let mut chunk = [0_u8; 4096];
        let mut keep = true;
        loop {
            match stream.read(&mut chunk) {
                Ok(0) => {
                    keep = false;
                    break;
                }
                Ok(count) => self.ipc_buf.extend_from_slice(&chunk[..count]),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => {
                    keep = false;
                    break;
                }
            }
            if self.ipc_buf.len() > 1_048_576 {
                self.ipc_buf.clear();
                break;
            }
        }
        if keep {
            self.ipc = Some(stream);
        }
        while let Some(split) = self.ipc_buf.iter().position(|byte| *byte == b'\n') {
            let line: Vec<u8> = self.ipc_buf.drain(..=split).collect();
            let line = String::from_utf8_lossy(&line);
            let Ok(value) = serde_json::from_str::<serde_json::Value>(line.trim()) else {
                continue;
            };
            let is_event =
                value.get("event").and_then(|event| event.as_str()) == Some("property-change");
            if !is_event {
                continue;
            }
            if value.get("name").and_then(|name| name.as_str()) != Some("user-data/dee-nice/event")
            {
                continue;
            }
            let Some(data) = value.get("data").and_then(|data| data.as_str()) else {
                continue;
            };
            if let Some(event) = classify_event(data) {
                events.push(event);
            }
        }
    }

    fn enqueue(&mut self, command: serde_json::Value) {
        self.pending.push(command);
        self.flush();
    }

    fn flush(&mut self) {
        if self.ipc.is_none() {
            return;
        }
        let pending = std::mem::take(&mut self.pending);
        for command in pending {
            self.send(&command);
        }
    }

    fn send(&mut self, value: &serde_json::Value) {
        let Some(stream) = self.ipc.as_mut() else {
            return;
        };
        let Ok(text) = serde_json::to_string(value) else {
            return;
        };
        let _ = stream.write_all(text.as_bytes());
        let _ = stream.write_all(b"\n");
        let _ = stream.flush();
    }
}

struct SlotLaunch<'a> {
    binary: &'a Path,
    display_name: &'a str,
    geometry: &'a str,
    muted: bool,
    directory: &'a Path,
    input_conf: &'a Path,
    script: &'a Path,
    index: u32,
    slot: Slot,
    pending: Option<serde_json::Value>,
}

fn spawn_slot(launch: &SlotLaunch<'_>) -> Result<SlotPlayer, String> {
    let ipc_path = short_socket_path(launch.index);
    let _ = fs::remove_file(&ipc_path);
    let arguments = playback::slot_arguments(
        launch.display_name,
        launch.geometry,
        launch.muted,
        &ipc_path,
        launch.input_conf,
        launch.script,
    );
    let child = Command::new(launch.binary)
        .args(arguments)
        .current_dir(launch.directory)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("could not start a slot: {error}"))?;
    Ok(SlotPlayer {
        slot: launch.slot,
        child,
        ipc: None,
        ipc_buf: Vec::new(),
        ipc_path,
        pending: launch.pending.clone().into_iter().collect(),
        clips_installed: false,
    })
}

fn awake_arguments(pid: u32) -> Vec<String> {
    ["-d", "-i", "-s", "-w", &pid.to_string()]
        .into_iter()
        .map(str::to_string)
        .collect()
}

/// Keeps the display on and blocks idle sleep until `pid` exits.
///
/// A missing `caffeinate` does not stop the show. Leaving display mode ends
/// the assertion because it is tied to the player process.
fn keep_awake(pid: u32) -> Option<Child> {
    let binary = Path::new("/usr/bin/caffeinate");
    if !binary.is_file() {
        return None;
    }
    Command::new(binary)
        .args(awake_arguments(pid))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok()
}

fn action_slot(action: &BindingAction) -> Option<Slot> {
    match action {
        BindingAction::Play { slot, .. } | BindingAction::Black { slot } => Some(*slot),
        BindingAction::ToggleLock | BindingAction::ExitDisplay => None,
    }
}

fn join_both(first: JoinHandle<()>, second: JoinHandle<()>) -> JoinHandle<()> {
    std::thread::spawn(move || {
        let _ = first.join();
        let _ = second.join();
    })
}

fn spawn_output_reader(
    pipe: impl std::io::Read + Send + 'static,
    log: Arc<Mutex<String>>,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        let mut reader = std::io::BufReader::new(pipe);
        let mut line = String::new();
        loop {
            line.clear();
            match std::io::BufRead::read_line(&mut reader, &mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    let Ok(mut guard) = log.lock() else {
                        break;
                    };
                    guard.push_str(&line);
                    if guard.len() > 8_000 {
                        let extra = guard.len() - 8_000;
                        guard.drain(..extra);
                    }
                }
            }
        }
    })
}

#[derive(Debug)]
enum ShowEvent {
    Lock,
    Play(String),
}

fn auto_lock_due(locked: bool, limit: Option<Duration>, idle: Duration) -> bool {
    limit.is_some_and(|limit| !locked && idle >= limit)
}

fn classify_event(data: &str) -> Option<ShowEvent> {
    let head = data.split(':').next()?;
    if head == "lock" {
        return Some(ShowEvent::Lock);
    }
    let key = head.strip_prefix("play ")?;
    if key.is_empty() {
        None
    } else {
        Some(ShowEvent::Play(key.to_string()))
    }
}

fn stop_message(status: std::process::ExitStatus, stderr: &str) -> String {
    let stderr = stderr.trim();
    if status.success() && stderr.is_empty() {
        return "Left display mode.".to_string();
    }
    let code = status
        .code()
        .map(|code| format!("mpv exited ({code})"))
        .unwrap_or_else(|| "mpv was stopped".to_string());
    if stderr.is_empty() {
        code
    } else {
        format!("{code}: {stderr}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staying_awake_follows_the_player_until_it_exits() {
        assert_eq!(
            awake_arguments(42),
            ["-d", "-i", "-s", "-w", "42"]
                .into_iter()
                .map(str::to_string)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn auto_lock_waits_until_the_idle_interval_and_stays_off_by_default() {
        let minute = Duration::from_secs(60);
        assert!(!auto_lock_due(false, None, minute));
        assert!(!auto_lock_due(true, Some(minute), minute));
        assert!(!auto_lock_due(false, Some(minute), Duration::from_secs(59)));
        assert!(auto_lock_due(false, Some(minute), minute));
    }

    #[test]
    fn play_and_lock_events_stay_distinct() {
        assert!(matches!(classify_event("lock:3"), Some(ShowEvent::Lock)));
        match classify_event("play a:4") {
            Some(ShowEvent::Play(key)) => assert_eq!(key, "a"),
            other => panic!("expected the played key, got {other:?}"),
        }
        assert!(classify_event("play :4").is_none());
    }

    #[test]
    #[ignore = "requires mpv on PATH or DEE_NICE_MPV"]
    fn mpv_accepts_the_display_flags_and_answers_ipc() {
        let binary = playback::mpv_binary().expect("mpv binary");
        let listed = Command::new(&binary)
            .arg("--list-options")
            .output()
            .expect("list options");
        let text = String::from_utf8_lossy(&listed.stdout);
        for flag in [
            "screen-name",
            "fs-screen-name",
            "force-window",
            "input-ipc-server",
            "input-default-bindings",
            "loop-file",
        ] {
            assert!(text.contains(flag), "mpv is missing {flag}");
        }

        let socket = short_socket_path(0);
        let _ = fs::remove_file(&socket);
        let mut child = Command::new(&binary)
            .args([
                "--no-config",
                "--idle=yes",
                "--force-window=no",
                "--vo=null",
                "--ao=null",
                "--no-input-default-bindings",
            ])
            .arg(format!("--input-ipc-server={}", socket.display()))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn mpv");
        let mut connected = None;
        for _ in 0..50 {
            if child.try_wait().ok().flatten().is_some() {
                break;
            }
            if let Ok(stream) = std::os::unix::net::UnixStream::connect(&socket) {
                connected = Some(stream);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let mut stream = connected.expect("ipc socket");
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(2)))
            .unwrap();
        stream
            .write_all(b"{\"command\":[\"get_version\"]}\n")
            .unwrap();
        let mut answer = String::new();
        let mut reader = std::io::BufReader::new(stream);
        std::io::BufRead::read_line(&mut reader, &mut answer).unwrap();
        assert!(answer.contains("success"), "{answer}");
        let _ = child.kill();
        let _ = child.wait();
        let _ = fs::remove_file(&socket);
    }

    /// Drives a real windowed mpv through the same session the app uses.
    /// Needs `DEE_NICE_MPV`, the demo clips, and a display. It fullscreens.
    #[test]
    #[ignore = "needs DEE_NICE_MPV, /tmp/dee-nice-demo, and a display"]
    fn display_mode_switches_locks_and_quits() {
        let _binary = playback::mpv_binary().expect("set DEE_NICE_MPV to a windowed mpv");
        let clips = Path::new("/tmp/dee-nice-demo");
        assert!(clips.join("red.mp4").is_file(), "missing red.mp4");
        assert!(clips.join("blue.mp4").is_file(), "missing blue.mp4");
        let display = main_screen_name();

        let mut show = Show::new(clips).expect("show");
        show.set_muted(true);
        show.set_output_display(Some(
            dee_nice_core::OutputDisplay::new(display, 0).expect("display"),
        ));
        show.assign(
            dee_nice_core::Key::from_name("a").expect("a"),
            "red.mp4",
            Slot::Full,
        )
        .expect("assign a");
        show.assign(
            dee_nice_core::Key::from_name("b").expect("b"),
            "blue.mp4",
            Slot::Full,
        )
        .expect("assign b");

        let mut session = MpvSession::start(&show, &BTreeMap::new(), (1920, 1080)).expect("start");
        let mut connected = false;
        for _ in 0..40 {
            let _ = session.poll();
            if session.ipc.is_some() {
                connected = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert!(
            connected,
            "mpv ipc did not connect: {}",
            session.stderr_text()
        );

        let fullscreen = ipc_request(
            &mut session,
            serde_json::json!(["get_property", "fullscreen"]),
        );
        assert_eq!(
            fullscreen.get("data"),
            Some(&serde_json::json!(true)),
            "{fullscreen}"
        );

        ipc_request(&mut session, serde_json::json!(["keypress", "a"]));
        let red = wait_path(&mut session, "red.mp4");
        ipc_request(&mut session, serde_json::json!(["keypress", "b"]));
        let blue = wait_path(&mut session, "blue.mp4");
        assert!(red.ends_with("red.mp4"), "{red}");
        assert!(blue.ends_with("blue.mp4"), "{blue}");

        ipc_request(&mut session, serde_json::json!(["keypress", "ctrl+L"]));
        let mut locked = false;
        for _ in 0..20 {
            let _ = session.poll();
            if session.locked {
                locked = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert!(
            locked,
            "Ctrl+Shift+L did not lock: {}",
            session.stderr_text()
        );

        ipc_request(&mut session, serde_json::json!(["keypress", "a"]));
        std::thread::sleep(std::time::Duration::from_millis(200));
        let still = ipc_request(&mut session, serde_json::json!(["get_property", "path"]));
        let still_path = still
            .get("data")
            .and_then(|data| data.as_str())
            .unwrap_or("");
        assert!(
            still_path.ends_with("blue.mp4"),
            "locked key still switched: {still}"
        );

        ipc_request(&mut session, serde_json::json!(["keypress", "ctrl+K"]));
        let mut left = None;
        for _ in 0..20 {
            if let SessionUpdate::Stopped(message) = session.poll() {
                left = Some(message);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let message = left.expect("mpv did not quit");
        assert_eq!(message, "Left display mode.");
    }

    fn main_screen_name() -> String {
        let output = Command::new("swift")
            .args(["-e", "import AppKit; print(NSScreen.main!.localizedName)"])
            .output()
            .expect("swift");
        let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
        assert!(!name.is_empty(), "no main screen");
        name
    }

    fn wait_path(session: &mut MpvSession, suffix: &str) -> String {
        for _ in 0..20 {
            let value = ipc_request(session, serde_json::json!(["get_property", "path"]));
            if let Some(path) = value.get("data").and_then(|data| data.as_str())
                && path.ends_with(suffix)
            {
                return path.to_string();
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        panic!("path never became {suffix}: {}", session.stderr_text());
    }

    fn ipc_request(session: &mut MpvSession, command: serde_json::Value) -> serde_json::Value {
        let mut stream = session.ipc.take().expect("ipc");
        stream.set_nonblocking(false).expect("blocking");
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(3)))
            .expect("timeout");
        let mut buffer = std::mem::take(&mut session.ipc_buf);
        let mut skipped = Vec::new();
        let payload = serde_json::json!({"request_id": 42, "command": command});
        let mut text = serde_json::to_string(&payload).expect("json");
        text.push('\n');
        stream.write_all(text.as_bytes()).expect("write");
        stream.flush().expect("flush");
        let started = std::time::Instant::now();
        loop {
            if let Some(split) = buffer.iter().position(|byte| *byte == b'\n') {
                let line: Vec<u8> = buffer.drain(..=split).collect();
                let parsed = String::from_utf8_lossy(&line);
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(parsed.trim())
                    && value.get("request_id").and_then(|id| id.as_i64()) == Some(42)
                {
                    skipped.extend_from_slice(&buffer);
                    session.ipc_buf = skipped;
                    stream.set_nonblocking(true).expect("nonblocking");
                    session.ipc = Some(stream);
                    return value;
                }
                skipped.extend_from_slice(&line);
                continue;
            }
            if started.elapsed() > std::time::Duration::from_secs(3) {
                panic!("ipc timeout: {}", String::from_utf8_lossy(&buffer));
            }
            let mut chunk = [0_u8; 4096];
            match stream.read(&mut chunk) {
                Ok(0) => panic!("ipc closed"),
                Ok(count) => buffer.extend_from_slice(&chunk[..count]),
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) => panic!("{error}"),
            }
        }
    }
}
