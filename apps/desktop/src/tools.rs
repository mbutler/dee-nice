//! Where the shell finds mpv, ffmpeg, and ffprobe.
//!
//! A copy beside this executable wins, including an mpv.app nested in the same
//! folder. Then the `DEE_NICE_*` override. `PATH` is the developer fallback.

use std::path::{Path, PathBuf};

pub fn find_tool(env_var: &str, file_name: &str) -> Option<PathBuf> {
    if let Some(path) = bundled_tool(file_name) {
        return Some(path);
    }
    if let Some(path) = env_file(env_var) {
        return Some(path);
    }
    on_path(file_name)
}

fn bundled_tool(file_name: &str) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    bundled_in(dir, file_name)
}

pub(crate) fn bundled_in(dir: &Path, file_name: &str) -> Option<PathBuf> {
    let sibling = dir.join(file_name);
    if sibling.is_file() {
        return Some(sibling);
    }
    let nested = dir
        .join(format!("{file_name}.app"))
        .join("Contents")
        .join("MacOS")
        .join(file_name);
    nested.is_file().then_some(nested)
}

fn env_file(var: &str) -> Option<PathBuf> {
    let path = PathBuf::from(std::env::var_os(var)?);
    path.is_file().then_some(path)
}

fn on_path(file_name: &str) -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    for directory in std::env::split_paths(&paths) {
        let candidate = directory.join(file_name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
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
            let path = std::env::temp_dir().join(format!("dee-nice-tools-{tick}-{n}"));
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
    fn a_sibling_tool_wins_over_a_nested_app() {
        let dir = TempDir::new();
        let sibling = dir.0.join("mpv");
        fs::write(&sibling, b"sibling").unwrap();
        let nested = dir.0.join("mpv.app/Contents/MacOS");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("mpv"), b"nested").unwrap();
        assert_eq!(bundled_in(&dir.0, "mpv"), Some(sibling));
    }

    #[test]
    fn a_nested_app_is_the_bundled_tool_when_there_is_no_sibling() {
        let dir = TempDir::new();
        let nested = dir.0.join("mpv.app/Contents/MacOS/mpv");
        fs::create_dir_all(nested.parent().unwrap()).unwrap();
        fs::write(&nested, b"nested").unwrap();
        assert_eq!(bundled_in(&dir.0, "mpv"), Some(nested));
        assert_eq!(bundled_in(&dir.0, "ffmpeg"), None);
    }
}
