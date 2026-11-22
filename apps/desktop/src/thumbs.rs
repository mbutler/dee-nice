//! Small preview frames for the setup window.
//!
//! Previews live in Application Support. The video folder is only read.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::SystemTime;

use crate::derivatives::ffmpeg_binary;
use crate::media;

pub struct Gallery {
    cache_dir: PathBuf,
    ready: BTreeMap<String, PathBuf>,
    queue: VecDeque<Pending>,
    failed: BTreeSet<String>,
    job: Option<Job>,
    missing_tool: bool,
}

struct Pending {
    clip: String,
    source: PathBuf,
    id: String,
    seek: &'static str,
}

struct Job {
    child: Child,
    clip: String,
    source: PathBuf,
    id: String,
    seek: &'static str,
    partial: PathBuf,
    output: PathBuf,
}

impl Gallery {
    pub fn open(cache_dir: PathBuf) -> Self {
        Self {
            cache_dir,
            ready: BTreeMap::new(),
            queue: VecDeque::new(),
            failed: BTreeSet::new(),
            job: None,
            missing_tool: false,
        }
    }

    pub fn path(&self, clip: &str) -> Option<&Path> {
        self.ready.get(clip).map(PathBuf::as_path)
    }

    pub fn busy(&self) -> bool {
        self.job.is_some() || !self.queue.is_empty()
    }

    /// True once, when ffmpeg cannot be found.
    pub fn take_missing_tool(&mut self) -> bool {
        if self.missing_tool {
            self.missing_tool = false;
            true
        } else {
            false
        }
    }

    pub fn sync(&mut self, directory: &Path, clips: &[String]) {
        let mut wanted = BTreeSet::new();
        let mut ready = BTreeMap::new();
        let mut queue = VecDeque::new();
        for clip in clips {
            let Ok(source) = media::resolve_clip(directory, clip) else {
                continue;
            };
            let Ok(meta) = fs::metadata(&source) else {
                continue;
            };
            let id = cache_key(&source, meta.len(), modified_nanos(meta.modified().ok()));
            wanted.insert(id.clone());
            let output = self.cache_dir.join(format!("{id}.png"));
            if output.is_file() {
                ready.insert(clip.clone(), output);
                continue;
            }
            if self.failed.contains(&id) {
                continue;
            }
            if self.job.as_ref().is_some_and(|job| job.id == id) {
                continue;
            }
            queue.push_back(Pending {
                clip: clip.clone(),
                source,
                id,
                seek: "0.5",
            });
        }
        self.ready = ready;
        self.queue = queue;
        self.failed.retain(|id| wanted.contains(id));
        if self
            .job
            .as_ref()
            .is_some_and(|job| !wanted.contains(&job.id))
        {
            self.stop_job();
        }
        self.prune(&wanted);
    }

    pub fn poll(&mut self) -> bool {
        let status = match self.job.as_mut() {
            Some(job) => match job.child.try_wait() {
                Ok(status) => status,
                Err(_) => {
                    self.stop_job();
                    self.pump();
                    return true;
                }
            },
            None => None,
        };
        let mut changed = false;
        if let Some(status) = status
            && let Some(job) = self.job.take()
        {
            changed = true;
            let wrote = status.success()
                && job.partial.is_file()
                && fs::rename(&job.partial, &job.output).is_ok();
            if wrote {
                self.ready.insert(job.clip, job.output);
            } else if job.seek == "0.5" {
                let _ = fs::remove_file(&job.partial);
                self.queue.push_front(Pending {
                    clip: job.clip,
                    source: job.source,
                    id: job.id,
                    seek: "0",
                });
            } else {
                let _ = fs::remove_file(&job.partial);
                self.failed.insert(job.id);
            }
        }
        changed || self.pump()
    }

    fn pump(&mut self) -> bool {
        if self.job.is_some() {
            return false;
        }
        let Some(pending) = self.queue.pop_front() else {
            return false;
        };
        let Some(ffmpeg) = ffmpeg_binary() else {
            self.missing_tool = true;
            self.queue.clear();
            return false;
        };
        if fs::create_dir_all(&self.cache_dir).is_err() {
            self.failed.insert(pending.id);
            return false;
        }
        let partial = self.cache_dir.join(format!("{}.part.png", pending.id));
        let output = self.cache_dir.join(format!("{}.png", pending.id));
        let child = Command::new(ffmpeg)
            .arg("-hide_banner")
            .arg("-loglevel")
            .arg("error")
            .arg("-nostdin")
            .arg("-y")
            .arg("-ss")
            .arg(pending.seek)
            .arg("-i")
            .arg(&pending.source)
            .arg("-frames:v")
            .arg("1")
            .arg("-vf")
            .arg("scale=240:-2")
            .arg(&partial)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        let Ok(child) = child else {
            self.failed.insert(pending.id);
            return false;
        };
        self.job = Some(Job {
            child,
            clip: pending.clip,
            source: pending.source,
            id: pending.id,
            seek: pending.seek,
            partial,
            output,
        });
        true
    }

    fn stop_job(&mut self) {
        if let Some(mut job) = self.job.take() {
            let _ = job.child.kill();
            let _ = job.child.wait();
            let _ = fs::remove_file(job.partial);
        }
    }

    fn prune(&self, wanted: &BTreeSet<String>) {
        let Ok(entries) = fs::read_dir(&self.cache_dir) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            let Some(id) = name
                .strip_suffix(".part.png")
                .or_else(|| name.strip_suffix(".png"))
            else {
                continue;
            };
            if !wanted.contains(id) {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
}

impl Drop for Gallery {
    fn drop(&mut self) {
        self.stop_job();
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

fn modified_nanos(modified: Option<SystemTime>) -> u128 {
    modified
        .and_then(|time| time.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::UNIX_EPOCH;

    static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let n = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "dee-nice-thumbs-{}-{}-{n}",
                std::process::id(),
                UNIX_EPOCH.elapsed().unwrap_or_default().as_nanos()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_saved_thumbnail_is_reused_and_old_ones_are_removed() {
        let root = TempDir::new();
        let media = root.0.join("videos");
        let cache = root.0.join("thumbs");
        fs::create_dir_all(&media).unwrap();
        fs::create_dir_all(&cache).unwrap();
        let source = media.join("red.mp4");
        fs::write(&source, b"not a video").unwrap();
        let meta = fs::metadata(&source).unwrap();
        let id = cache_key(
            &source.canonicalize().unwrap(),
            meta.len(),
            modified_nanos(meta.modified().ok()),
        );
        fs::write(cache.join(format!("{id}.png")), b"png").unwrap();
        fs::write(cache.join("old.png"), b"old").unwrap();

        let mut gallery = Gallery::open(cache.clone());
        gallery.sync(&media, &["red.mp4".to_string()]);

        assert!(gallery.path("red.mp4").is_some());
        assert!(!cache.join("old.png").exists());
        assert!(!gallery.busy());
    }
}
