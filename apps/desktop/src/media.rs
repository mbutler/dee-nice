use std::fs;
use std::path::{Path, PathBuf};

const VIDEO_EXTENSIONS: &[&str] = &[
    "avi", "flv", "m2ts", "m4v", "mkv", "mov", "mp4", "mpeg", "mpg", "mts", "webm", "wmv",
];
const MAX_CLIPS: usize = 2000;
const MAX_DEPTH: u32 = 8;

#[derive(Debug, PartialEq, Eq)]
pub struct ClipList {
    pub clips: Vec<String>,
    pub truncated: bool,
}

#[derive(Debug)]
pub enum ScanError {
    Missing,
    Unreadable(String),
}

impl std::fmt::Display for ScanError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing => write!(formatter, "the media folder does not exist"),
            Self::Unreadable(detail) => {
                write!(formatter, "the media folder could not be read: {detail}")
            }
        }
    }
}

/// Relative video paths inside `directory`, using `/` between components.
pub fn list_clips(directory: &Path) -> Result<ClipList, ScanError> {
    if !directory.is_dir() {
        return Err(ScanError::Missing);
    }
    let mut clips = Vec::new();
    let mut truncated = false;
    walk(directory, directory, 0, &mut clips, &mut truncated)?;
    clips.sort();
    Ok(ClipList { clips, truncated })
}

fn walk(
    root: &Path,
    directory: &Path,
    depth: u32,
    clips: &mut Vec<String>,
    truncated: &mut bool,
) -> Result<(), ScanError> {
    if *truncated || depth > MAX_DEPTH {
        *truncated = true;
        return Ok(());
    }
    let entries =
        fs::read_dir(directory).map_err(|error| ScanError::Unreadable(error.to_string()))?;
    for entry in entries {
        if clips.len() >= MAX_CLIPS {
            *truncated = true;
            return Ok(());
        }
        let entry = entry.map_err(|error| ScanError::Unreadable(error.to_string()))?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| ScanError::Unreadable(error.to_string()))?;
        if file_type.is_dir() {
            walk(root, &path, depth + 1, clips, truncated)?;
        } else if is_video_name(name)
            && path.is_file()
            && let Some(relative) = relative_clip(root, &path)
        {
            clips.push(relative);
        }
    }
    Ok(())
}

fn is_video_name(name: &str) -> bool {
    let Some((_, extension)) = name.rsplit_once('.') else {
        return false;
    };
    VIDEO_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
}

fn relative_clip(root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    let mut parts = Vec::new();
    for component in relative.components() {
        let part = component.as_os_str().to_str()?;
        parts.push(part);
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("/"))
    }
}

#[derive(Debug)]
pub enum ResolveError {
    Missing { clip: String },
    Escapes { clip: String },
    NotUnicode { clip: String },
}

impl std::fmt::Display for ResolveError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing { clip } => write!(formatter, "missing clip {clip}"),
            Self::Escapes { clip } => {
                write!(formatter, "clip {clip} points outside the media folder")
            }
            Self::NotUnicode { clip } => write!(formatter, "clip {clip} is not valid Unicode"),
        }
    }
}

/// Canonical path of a clip that stays inside the media folder.
pub fn resolve_clip(directory: &Path, clip: &str) -> Result<PathBuf, ResolveError> {
    let joined = directory.join(clip);
    let directory = directory
        .canonicalize()
        .map_err(|_| ResolveError::Missing {
            clip: clip.to_string(),
        })?;
    let file = joined.canonicalize().map_err(|_| ResolveError::Missing {
        clip: clip.to_string(),
    })?;
    if !file.starts_with(&directory) || !file.is_file() {
        return Err(ResolveError::Escapes {
            clip: clip.to_string(),
        });
    }
    if file.to_str().is_none() {
        return Err(ResolveError::NotUnicode {
            clip: clip.to_string(),
        });
    }
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;
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
            let path = std::env::temp_dir().join(format!("dee-nice-media-{tick}-{n}"));
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
    fn scan_finds_nested_videos_and_skips_hidden_and_other_files() {
        let dir = TempDir::new();
        fs::create_dir(dir.0.join("loops")).unwrap();
        fs::write(dir.0.join("loops/a.MP4"), b"video").unwrap();
        fs::write(dir.0.join("notes.txt"), b"nope").unwrap();
        fs::create_dir(dir.0.join(".hidden")).unwrap();
        fs::write(dir.0.join(".hidden/secret.mp4"), b"video").unwrap();
        let list = list_clips(&dir.0).unwrap();
        assert_eq!(list.clips, vec!["loops/a.MP4".to_string()]);
        assert!(!list.truncated);
    }

    #[test]
    fn a_missing_folder_is_not_an_empty_show() {
        let missing = std::env::temp_dir().join("dee-nice-missing-media-folder");
        let _ = fs::remove_dir_all(&missing);
        assert!(matches!(list_clips(&missing), Err(ScanError::Missing)));
    }

    #[test]
    fn resolve_accepts_a_file_inside_the_folder() {
        let dir = TempDir::new();
        fs::create_dir(dir.0.join("loops")).unwrap();
        fs::write(dir.0.join("loops/a.mp4"), b"video").unwrap();
        let resolved = resolve_clip(&dir.0, "loops/a.mp4").unwrap();
        let root = dir.0.canonicalize().unwrap();
        assert!(resolved.starts_with(&root));
        assert!(resolved.is_file());
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_outside_the_folder_is_rejected() {
        let dir = TempDir::new();
        let outside = TempDir::new();
        fs::write(outside.0.join("outside.mp4"), b"video").unwrap();
        std::os::unix::fs::symlink(outside.0.join("outside.mp4"), dir.0.join("link.mp4")).unwrap();
        assert!(matches!(
            resolve_clip(&dir.0, "link.mp4"),
            Err(ResolveError::Escapes { .. })
        ));
    }
}
