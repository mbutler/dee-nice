use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Instant, SystemTime};

use dee_nice_core::Show;
use serde::Deserialize;

use crate::media;

const EIGHT_GB: u64 = 8 * 1024 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MachineProfile {
    pub apple_silicon: bool,
    pub memory_bytes: u64,
    pub low_power: bool,
}

pub struct Library {
    cache_dir: PathBuf,
    profile: MachineProfile,
    ffmpeg: Option<PathBuf>,
    ffprobe: Option<PathBuf>,
    known: BTreeMap<PathBuf, KnownSource>,
    wanted: Vec<Want>,
    ready: BTreeMap<String, PathBuf>,
    job: Option<Job>,
    failed: BTreeSet<String>,
    failures: Vec<String>,
    missing_tool: bool,
    profile_checked: Instant,
}

#[derive(Clone)]
struct KnownSource {
    len: u64,
    modified: u128,
    canonical: PathBuf,
    codec: String,
    width: u32,
    height: u32,
}

#[derive(Clone)]
struct Want {
    relative: String,
    source: PathBuf,
    id: String,
    width: u32,
    height: u32,
    fit: u32,
}

struct Job {
    id: String,
    relative: String,
    partial: PathBuf,
    output: PathBuf,
    manifest: PathBuf,
    fit: u32,
    child: Child,
    stderr: Arc<Mutex<String>>,
    stderr_thread: Option<JoinHandle<()>>,
}

#[derive(Deserialize)]
struct ProbeFile {
    streams: Vec<ProbeStream>,
}

#[derive(Deserialize)]
struct ProbeStream {
    codec_name: String,
    width: u32,
    height: u32,
}

#[derive(Deserialize)]
struct Manifest {
    fit: u32,
}

impl Library {
    pub fn open(cache_dir: PathBuf) -> Self {
        Self {
            cache_dir,
            profile: detect_profile(),
            ffmpeg: ffmpeg_binary(),
            ffprobe: ffprobe_binary(),
            known: BTreeMap::new(),
            wanted: Vec::new(),
            ready: BTreeMap::new(),
            job: None,
            failed: BTreeSet::new(),
            failures: Vec::new(),
            missing_tool: false,
            profile_checked: Instant::now(),
        }
    }

    /// True when Low Power Mode or the machine cap changed since the last check.
    pub fn profile_changed(&mut self) -> bool {
        if self.profile_checked.elapsed() < std::time::Duration::from_secs(2) {
            return false;
        }
        self.profile_checked = Instant::now();
        let next = detect_profile();
        let changed = machine_cap(&next) != machine_cap(&self.profile)
            || next.low_power != self.profile.low_power;
        self.profile = next;
        changed
    }

    pub fn ready(&self) -> &BTreeMap<String, PathBuf> {
        &self.ready
    }

    pub fn busy(&self) -> bool {
        self.job.is_some()
            || self.wanted.iter().any(|want| {
                !self.ready.contains_key(&want.relative) && !self.failed.contains(&want.id)
            })
    }

    pub fn status_line(&self, display_long_edge: Option<u32>) -> String {
        if self.ffmpeg.is_none() || self.ffprobe.is_none() {
            return "ffmpeg was not found, so originals will play.".to_string();
        }
        let edge = match display_long_edge {
            Some(edge) => fit_edge(machine_cap(&self.profile), edge),
            None => machine_cap(&self.profile),
        };
        status_line(edge)
    }

    pub fn preparing(&self) -> Option<String> {
        if let Some(job) = &self.job {
            return Some(file_name(&job.relative));
        }
        self.wanted.iter().find_map(|want| {
            if self.ready.contains_key(&want.relative) || self.failed.contains(&want.id) {
                None
            } else {
                Some(file_name(&want.relative))
            }
        })
    }

    pub fn take_failures(&mut self) -> Vec<String> {
        std::mem::take(&mut self.failures)
    }

    pub fn sync(&mut self, show: &Show, display_long_edge: Option<u32>) {
        self.profile = detect_profile();
        let Some(display_long_edge) = display_long_edge else {
            self.stop_unwanted("");
            self.wanted.clear();
            self.ready.clear();
            return;
        };
        if self.ffmpeg.is_none() || self.ffprobe.is_none() {
            self.stop_job();
            if !self.missing_tool {
                self.failures
                    .push("ffmpeg was not found, so originals will play.".to_string());
                self.missing_tool = true;
            }
            self.ready.clear();
            self.wanted.clear();
            return;
        }
        let fit = fit_edge(machine_cap(&self.profile), display_long_edge);
        let mut clips = BTreeSet::new();
        for (_, clip) in show.assignments() {
            clips.insert(clip.to_string());
        }
        let mut wanted = Vec::new();
        let mut ready = BTreeMap::new();
        for relative in clips {
            match self.consider(show, &relative, fit) {
                Consider::Ready(path) => {
                    ready.insert(relative, path);
                }
                Consider::Encode(want) => wanted.push(want),
                Consider::Source | Consider::Skip => {}
            }
        }
        let mut keep: BTreeSet<_> = wanted.iter().map(|want| want.id.clone()).collect();
        for path in ready.values() {
            if let Some(id) = path.parent().and_then(|dir| dir.file_name()) {
                keep.insert(id.to_string_lossy().into_owned());
            }
        }
        retain_cache(&self.cache_dir, &keep);
        let current = self.job.as_ref().map(|job| job.id.clone());
        if let Some(id) = current
            && !keep.contains(&id)
        {
            self.stop_job();
        }
        self.failed.retain(|id| keep.contains(id));
        self.wanted = wanted;
        self.ready = ready;
        self.pump();
    }

    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        if let Some(job) = self.job.as_mut()
            && let Ok(Some(status)) = job.child.try_wait()
        {
            let job = self.job.take().expect("job");
            if let Some(thread) = job.stderr_thread {
                let _ = thread.join();
            }
            if status.success() && job.partial.is_file() {
                if fs::rename(&job.partial, &job.output).is_ok()
                    && write_manifest(&job.manifest, job.fit).is_ok()
                {
                    self.ready.insert(job.relative, job.output);
                    changed = true;
                } else {
                    self.failures.push(format!(
                        "Could not store the lighter copy of {}.",
                        file_name(&job.relative)
                    ));
                    self.failed.insert(job.id);
                }
            } else {
                let _ = fs::remove_file(&job.partial);
                let detail = job
                    .stderr
                    .lock()
                    .map(|guard| guard.trim().to_string())
                    .unwrap_or_default();
                let detail = if detail.is_empty() {
                    format!("ffmpeg exited ({})", status.code().unwrap_or(1))
                } else {
                    detail
                };
                self.failures.push(format!(
                    "Could not make a lighter copy of {}: {detail}",
                    file_name(&job.relative)
                ));
                self.failed.insert(job.id);
                changed = true;
            }
        }
        let started = self.pump();
        changed || started
    }

    fn consider(&mut self, show: &Show, relative: &str, fit: u32) -> Consider {
        let source = match media::resolve_clip(show.media_directory(), relative) {
            Ok(path) => path,
            Err(error) => {
                self.failures.push(error.to_string());
                return Consider::Skip;
            }
        };
        let meta = match fs::metadata(&source) {
            Ok(meta) => meta,
            Err(error) => {
                self.failures.push(format!("{}: {error}", source.display()));
                return Consider::Skip;
            }
        };
        let modified = modified_nanos(meta.modified().ok());
        let len = meta.len();
        let known = if let Some(known) = self.known.get(&source)
            && known.len == len
            && known.modified == modified
        {
            known.clone()
        } else {
            let Some(ffprobe) = self.ffprobe.clone() else {
                return Consider::Skip;
            };
            let canonical = source.canonicalize().unwrap_or_else(|_| source.clone());
            match probe(&ffprobe, &source) {
                Ok((codec, width, height)) => {
                    let facts = KnownSource {
                        len,
                        modified,
                        canonical,
                        codec,
                        width,
                        height,
                    };
                    self.known.insert(source.clone(), facts.clone());
                    facts
                }
                Err(error) => {
                    self.failures.push(format!(
                        "Could not inspect {}: {error}",
                        file_name(relative)
                    ));
                    return Consider::Skip;
                }
            }
        };
        let long_edge = known.width.max(known.height);
        if plays_source(&known.codec, long_edge, fit) {
            return Consider::Source;
        }
        let id = cache_key(&known.canonical, len, modified);
        if let Some(path) = cached_output(&self.cache_dir, &id, fit) {
            return Consider::Ready(path);
        }
        let stale = self.cache_dir.join(&id);
        let _ = fs::remove_file(stale.join("clip.mp4"));
        let _ = fs::remove_file(stale.join("manifest.json"));
        if self.failed.contains(&id) {
            return Consider::Skip;
        }
        let (width, height) = output_size(known.width, known.height, fit);
        Consider::Encode(Want {
            relative: relative.to_string(),
            source,
            id,
            width,
            height,
            fit,
        })
    }

    fn pump(&mut self) -> bool {
        if self.job.is_some() {
            return false;
        }
        let Some(ffmpeg) = self.ffmpeg.clone() else {
            return false;
        };
        let Some(index) = self.wanted.iter().position(|want| {
            !self.ready.contains_key(&want.relative) && !self.failed.contains(&want.id)
        }) else {
            return false;
        };
        let want = self.wanted[index].clone();
        match spawn_encode(&ffmpeg, &self.cache_dir, &want) {
            Ok(job) => {
                self.job = Some(job);
                true
            }
            Err(error) => {
                self.failures.push(error);
                self.failed.insert(want.id);
                true
            }
        }
    }

    fn stop_unwanted(&mut self, keep: &str) {
        if self.job.as_ref().is_some_and(|job| job.id != keep) {
            self.stop_job();
        }
    }

    fn stop_job(&mut self) {
        if let Some(mut job) = self.job.take() {
            let _ = job.child.kill();
            let _ = job.child.wait();
            if let Some(thread) = job.stderr_thread.take() {
                let _ = thread.join();
            }
            let _ = fs::remove_file(&job.partial);
        }
    }
}

impl Drop for Library {
    fn drop(&mut self) {
        self.stop_job();
    }
}

enum Consider {
    Source,
    Skip,
    Ready(PathBuf),
    Encode(Want),
}

pub fn machine_cap(profile: &MachineProfile) -> u32 {
    if profile.low_power || !profile.apple_silicon || profile.memory_bytes <= EIGHT_GB {
        1920
    } else {
        3840
    }
}

pub fn fit_edge(cap: u32, display_long_edge: u32) -> u32 {
    cap.min(display_long_edge.max(2))
}

pub fn plays_source(codec: &str, long_edge: u32, fit: u32) -> bool {
    let friendly = matches!(codec.to_ascii_lowercase().as_str(), "h264" | "avc" | "hevc");
    friendly && long_edge > 0 && long_edge <= fit
}

pub fn output_size(width: u32, height: u32, fit: u32) -> (u32, u32) {
    if width == 0 || height == 0 {
        return (2, 2);
    }
    let long_edge = width.max(height);
    let edge = long_edge.min(fit).max(2);
    let (width, height) = if width >= height {
        let scaled = (u64::from(height) * u64::from(edge) / u64::from(width)).max(2);
        (edge, u32::try_from(scaled).unwrap_or(2))
    } else {
        let scaled = (u64::from(width) * u64::from(edge) / u64::from(height)).max(2);
        (u32::try_from(scaled).unwrap_or(2), edge)
    };
    (even(width), even(height))
}

pub fn status_line(long_edge: u32) -> String {
    let label = if long_edge <= 1920 { "1080p" } else { "4K" };
    format!("This Mac plays lighter copies, up to {label}.")
}

pub fn low_power_enabled(pmset: &str) -> bool {
    pmset.lines().any(|line| {
        let mut parts = line.split_whitespace();
        parts.next() == Some("lowpowermode") && parts.next() == Some("1")
    })
}

fn even(value: u32) -> u32 {
    let value = value.max(2);
    if value.is_multiple_of(2) {
        value
    } else {
        value - 1
    }
}

fn cache_key(canonical: &Path, len: u64, modified: u128) -> String {
    let text = format!("{}\n{len}\n{modified}", canonical.display());
    let mut high = 0xcbf29ce484222325u64;
    let mut low = 0x100000001b3u64;
    for byte in text.bytes() {
        high ^= u64::from(byte);
        high = high.wrapping_mul(0x100000001b3);
        low ^= u64::from(byte);
        low = low.wrapping_mul(0xcbf29ce484222325);
    }
    format!("{high:016x}{low:016x}")
}

fn cached_output(cache_dir: &Path, id: &str, fit: u32) -> Option<PathBuf> {
    let output = cache_dir.join(id).join("clip.mp4");
    let manifest = cache_dir.join(id).join("manifest.json");
    if !output.is_file() {
        return None;
    }
    let text = fs::read_to_string(manifest).ok()?;
    let parsed: Manifest = serde_json::from_str(&text).ok()?;
    (parsed.fit == fit).then_some(output)
}

fn retain_cache(cache_dir: &Path, keep: &BTreeSet<String>) {
    let Ok(entries) = fs::read_dir(cache_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !keep.contains(&name) {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}

fn write_manifest(path: &Path, fit: u32) -> Result<(), String> {
    let mut file = File::create(path).map_err(|error| error.to_string())?;
    let body = format!("{{\"fit\":{fit}}}\n");
    file.write_all(body.as_bytes())
        .map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())
}

fn spawn_encode(ffmpeg: &Path, cache_dir: &Path, want: &Want) -> Result<Job, String> {
    let dir = cache_dir.join(&want.id);
    fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let partial = dir.join("clip.part.mp4");
    let output = dir.join("clip.mp4");
    let manifest = dir.join("manifest.json");
    let _ = fs::remove_file(&partial);
    let mut child = Command::new(ffmpeg)
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-nostdin")
        .arg("-y")
        .arg("-i")
        .arg(&want.source)
        .arg("-map")
        .arg("0:v:0")
        .arg("-map")
        .arg("0:a:0?")
        .arg("-vf")
        .arg(format!("scale={}:{}", want.width, want.height))
        .arg("-c:v")
        .arg("h264_videotoolbox")
        .arg("-pix_fmt")
        .arg("yuv420p")
        .arg("-c:a")
        .arg("aac")
        .arg("-ac")
        .arg("2")
        .arg("-b:a")
        .arg("160k")
        .arg("-movflags")
        .arg("+faststart")
        .arg(&partial)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("could not start ffmpeg: {error}"))?;
    let stderr_pipe = child.stderr.take();
    let stderr = Arc::new(Mutex::new(String::new()));
    let stderr_thread = stderr_pipe.map(|pipe| {
        let log = Arc::clone(&stderr);
        std::thread::spawn(move || {
            let mut reader = std::io::BufReader::new(pipe);
            let mut line = String::new();
            loop {
                line.clear();
                match std::io::BufRead::read_line(&mut reader, &mut line) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        let Ok(mut guard) = log.lock() else { break };
                        guard.push_str(&line);
                        if guard.len() > 800 {
                            let extra = guard.len() - 800;
                            guard.drain(..extra);
                        }
                    }
                }
            }
        })
    });
    Ok(Job {
        id: want.id.clone(),
        relative: want.relative.clone(),
        partial,
        output,
        manifest,
        fit: want.fit,
        child,
        stderr,
        stderr_thread,
    })
}

fn probe(ffprobe: &Path, file: &Path) -> Result<(String, u32, u32), String> {
    let output = Command::new(ffprobe)
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=codec_name,width,height",
            "-of",
            "json",
        ])
        .arg(file)
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    let parsed: ProbeFile =
        serde_json::from_slice(&output.stdout).map_err(|error| error.to_string())?;
    let Some(stream) = parsed.streams.into_iter().next() else {
        return Err("the file has no video stream".to_string());
    };
    if stream.width == 0 || stream.height == 0 {
        return Err("the video size is unknown".to_string());
    }
    Ok((stream.codec_name, stream.width, stream.height))
}

fn detect_profile() -> MachineProfile {
    if !cfg!(target_os = "macos") {
        return MachineProfile {
            apple_silicon: false,
            memory_bytes: EIGHT_GB,
            low_power: false,
        };
    }
    MachineProfile {
        apple_silicon: sysctl("hw.optional.arm64").ok().as_deref() == Some("1"),
        memory_bytes: sysctl("hw.memsize")
            .ok()
            .and_then(|text| text.parse().ok())
            .unwrap_or(EIGHT_GB),
        low_power: Command::new("pmset")
            .arg("-g")
            .output()
            .ok()
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .is_some_and(|text| low_power_enabled(&text)),
    }
}

fn sysctl(name: &str) -> Result<String, ()> {
    let output = Command::new("sysctl")
        .args(["-n", name])
        .output()
        .map_err(|_| ())?;
    if !output.status.success() {
        return Err(());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub(crate) fn ffmpeg_binary() -> Option<PathBuf> {
    crate::tools::find_tool("DEE_NICE_FFMPEG", "ffmpeg")
}

pub(crate) fn ffprobe_binary() -> Option<PathBuf> {
    crate::tools::find_tool("DEE_NICE_FFPROBE", "ffprobe")
}

fn modified_nanos(modified: Option<SystemTime>) -> u128 {
    modified
        .and_then(|time| time.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos())
        .unwrap_or(0)
}

fn file_name(relative: &str) -> String {
    relative.rsplit('/').next().unwrap_or(relative).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(apple: bool, gigabytes: u64, low_power: bool) -> MachineProfile {
        MachineProfile {
            apple_silicon: apple,
            memory_bytes: gigabytes * 1024 * 1024 * 1024,
            low_power,
        }
    }

    #[test]
    fn the_cap_follows_the_mac_and_low_power_mode() {
        assert_eq!(machine_cap(&profile(false, 64, false)), 1920);
        assert_eq!(machine_cap(&profile(true, 8, false)), 1920);
        assert_eq!(machine_cap(&profile(true, 32, true)), 1920);
        assert_eq!(machine_cap(&profile(true, 16, false)), 3840);
        assert_eq!(machine_cap(&profile(true, 18, false)), 3840);
    }

    #[test]
    fn a_friendly_file_inside_the_fit_plays_as_itself() {
        assert!(plays_source("h264", 1920, 1920));
        assert!(plays_source("hevc", 1280, 1920));
        assert!(!plays_source("h264", 3840, 1920));
        assert!(!plays_source("prores", 1920, 1920));
        assert_eq!(output_size(3840, 2160, 1920), (1920, 1080));
        assert_eq!(output_size(1280, 720, 1920), (1280, 720));
        assert_eq!(fit_edge(3840, 1920), 1920);
        assert_eq!(
            status_line(1920),
            "This Mac plays lighter copies, up to 1080p."
        );
        assert_eq!(
            status_line(3840),
            "This Mac plays lighter copies, up to 4K."
        );
    }

    #[test]
    fn low_power_mode_is_read_from_pmset() {
        assert!(low_power_enabled("lowpowermode         1\n"));
        assert!(!low_power_enabled("lowpowermode         0\n"));
    }

    #[test]
    fn a_finished_copy_is_used_only_for_the_current_fit() {
        let dir = std::env::temp_dir().join(format!("dee-nice-cache-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("keep")).unwrap();
        fs::write(dir.join("keep").join("clip.mp4"), b"mp4").unwrap();
        fs::write(dir.join("keep").join("manifest.json"), "{\"fit\":1920}\n").unwrap();
        fs::create_dir_all(dir.join("old")).unwrap();
        fs::write(dir.join("old").join("clip.mp4"), b"mp4").unwrap();
        fs::write(dir.join("old").join("manifest.json"), "{\"fit\":3840}\n").unwrap();
        assert!(cached_output(&dir, "keep", 1920).is_some());
        assert!(cached_output(&dir, "keep", 3840).is_none());
        let mut keep = BTreeSet::new();
        keep.insert("keep".to_string());
        retain_cache(&dir, &keep);
        assert!(dir.join("keep").is_dir());
        assert!(!dir.join("old").exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
