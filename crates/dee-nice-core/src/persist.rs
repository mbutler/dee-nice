use std::fs::{self, File};
use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::ShowError;
use crate::show::{OutputDisplay, Show};

pub const SHOW_SCHEMA_VERSION: u32 = 2;
const LEGACY_SCHEMA_VERSION: u32 = 1;
const MAX_SHOW_BYTES: u64 = 1024 * 1024;

/// `<media>/.dee-nice/show.json`. The media folder is the parent of `.dee-nice`.
pub fn portable_show_path(media_directory: &Path) -> PathBuf {
    media_directory.join(".dee-nice").join("show.json")
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PortableShow {
    schema_version: u32,
    assignments: Vec<FileAssignment>,
    muted: bool,
    start_locked: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyShow {
    schema_version: u32,
    media_directory: String,
    assignments: Vec<FileAssignment>,
    muted: bool,
    start_locked: bool,
    output_display: Option<FileOutputDisplay>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileAssignment {
    key: String,
    clip: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileOutputDisplay {
    name: String,
    index: u32,
}

/// Writes a schema-2 show to a temporary file beside `path`, then replaces `path`.
///
/// The file stores the key map, mute, and start locked. The media directory and
/// the display stay out of the file. A failed write leaves an existing show
/// file in place.
pub fn save_show(path: &Path, show: &Show) -> Result<(), ShowError> {
    let bytes = serialize_show(show).map_err(|error| match error {
        ShowError::SaveFailed { detail, .. } => ShowError::SaveFailed {
            path: path.to_path_buf(),
            detail,
        },
        other => other,
    })?;
    atomic_write(path, &bytes)
}

/// Loads a show file. Schema 2 is the portable file inside `.dee-nice`. Schema
/// 1 is the older file that still carries a directory and a display. Any other
/// schema is rejected before its fields are interpreted.
pub fn load_show(path: &Path) -> Result<Show, ShowError> {
    let file = File::open(path).map_err(|error| ShowError::LoadFailed {
        path: path.to_path_buf(),
        detail: error.to_string(),
    })?;
    let bytes = file
        .metadata()
        .map_err(|error| ShowError::LoadFailed {
            path: path.to_path_buf(),
            detail: error.to_string(),
        })?
        .len();
    if bytes > MAX_SHOW_BYTES {
        return Err(ShowError::ShowFileTooLarge {
            path: path.to_path_buf(),
            bytes,
        });
    }
    let value: serde_json::Value =
        serde_json::from_reader(BufReader::new(file)).map_err(|error| {
            ShowError::InvalidShowFile {
                path: path.to_path_buf(),
                detail: error.to_string(),
            }
        })?;
    let schema_version =
        read_schema_version(&value).map_err(|detail| ShowError::InvalidShowFile {
            path: path.to_path_buf(),
            detail,
        })?;
    match schema_version {
        LEGACY_SCHEMA_VERSION => {
            let file_show: LegacyShow =
                serde_json::from_value(value).map_err(|error| ShowError::InvalidShowFile {
                    path: path.to_path_buf(),
                    detail: error.to_string(),
                })?;
            file_show.into_show(path)
        }
        SHOW_SCHEMA_VERSION => {
            let file_show: PortableShow =
                serde_json::from_value(value).map_err(|error| ShowError::InvalidShowFile {
                    path: path.to_path_buf(),
                    detail: error.to_string(),
                })?;
            file_show.into_show(path)
        }
        found => Err(ShowError::UnsupportedSchema { found }),
    }
}

fn read_schema_version(value: &serde_json::Value) -> Result<u32, String> {
    let Some(raw) = value.get("schema_version") else {
        return Err("schema_version is missing".to_string());
    };
    let Some(number) = raw.as_u64() else {
        return Err("schema_version is not a version number".to_string());
    };
    u32::try_from(number).map_err(|_| "schema_version is not a version number".to_string())
}

fn serialize_show(show: &Show) -> Result<Vec<u8>, ShowError> {
    let Show {
        media_directory: _,
        assignments,
        muted,
        start_locked,
        output_display: _,
    } = show;
    let mut pairs: Vec<_> = assignments.iter().collect();
    pairs.sort_by_key(|(left, _)| left.name());
    let assignments = pairs
        .into_iter()
        .map(|(key, clip)| FileAssignment {
            key: key.name(),
            clip: clip.clone(),
        })
        .collect();
    let file_show = PortableShow {
        schema_version: SHOW_SCHEMA_VERSION,
        assignments,
        muted: *muted,
        start_locked: *start_locked,
    };
    let mut bytes =
        serde_json::to_vec_pretty(&file_show).map_err(|error| ShowError::SaveFailed {
            path: PathBuf::new(),
            detail: error.to_string(),
        })?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn apply_assignments(
    show: &mut Show,
    path: &Path,
    assignments: Vec<FileAssignment>,
) -> Result<(), ShowError> {
    for FileAssignment { key, clip } in assignments {
        let Some(key) = crate::show::Key::from_name(&key) else {
            return Err(ShowError::InvalidShowFile {
                path: path.to_path_buf(),
                detail: format!("unknown key \"{key}\""),
            });
        };
        if show.clip(key).is_some() {
            return Err(ShowError::InvalidShowFile {
                path: path.to_path_buf(),
                detail: format!("key \"{}\" is assigned more than once", key.name()),
            });
        }
        show.assign(key, &clip)
            .map_err(|error| ShowError::InvalidShowFile {
                path: path.to_path_buf(),
                detail: error.to_string(),
            })?;
    }
    Ok(())
}

fn portable_media_directory(path: &Path) -> Result<PathBuf, String> {
    let Some(parent) = path.parent() else {
        return Err("the show file is not inside .dee-nice".to_string());
    };
    if parent.file_name().and_then(|name| name.to_str()) != Some(".dee-nice") {
        return Err("the show file is not inside .dee-nice".to_string());
    }
    match parent.parent() {
        Some(media) if !media.as_os_str().is_empty() => Ok(media.to_path_buf()),
        Some(_) | None => Ok(PathBuf::from(".")),
    }
}

impl PortableShow {
    fn into_show(self, path: &Path) -> Result<Show, ShowError> {
        let PortableShow {
            schema_version,
            assignments,
            muted,
            start_locked,
        } = self;
        if schema_version != SHOW_SCHEMA_VERSION {
            return Err(ShowError::UnsupportedSchema {
                found: schema_version,
            });
        }
        let media_directory =
            portable_media_directory(path).map_err(|detail| ShowError::InvalidShowFile {
                path: path.to_path_buf(),
                detail,
            })?;
        let mut show = Show::new(media_directory).map_err(|error| ShowError::InvalidShowFile {
            path: path.to_path_buf(),
            detail: error.to_string(),
        })?;
        show.set_muted(muted);
        show.set_start_locked(start_locked);
        apply_assignments(&mut show, path, assignments)?;
        Ok(show)
    }
}

impl LegacyShow {
    fn into_show(self, path: &Path) -> Result<Show, ShowError> {
        let LegacyShow {
            schema_version,
            media_directory,
            assignments,
            muted,
            start_locked,
            output_display,
        } = self;
        if schema_version != LEGACY_SCHEMA_VERSION {
            return Err(ShowError::UnsupportedSchema {
                found: schema_version,
            });
        }
        let mut show = Show::new(media_directory).map_err(|error| ShowError::InvalidShowFile {
            path: path.to_path_buf(),
            detail: error.to_string(),
        })?;
        show.set_muted(muted);
        show.set_start_locked(start_locked);
        if let Some(FileOutputDisplay { name, index }) = output_display {
            let display =
                OutputDisplay::new(name, index).map_err(|error| ShowError::InvalidShowFile {
                    path: path.to_path_buf(),
                    detail: error.to_string(),
                })?;
            show.set_output_display(Some(display));
        }
        apply_assignments(&mut show, path, assignments)?;
        Ok(show)
    }
}

fn atomic_write(destination: &Path, bytes: &[u8]) -> Result<(), ShowError> {
    let temp = temp_path(destination);
    let write_result = write_temp_and_replace(&temp, destination, bytes);
    if write_result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    write_result
}

fn write_temp_and_replace(temp: &Path, destination: &Path, bytes: &[u8]) -> Result<(), ShowError> {
    let mut file = File::create(temp).map_err(|error| ShowError::SaveFailed {
        path: destination.to_path_buf(),
        detail: error.to_string(),
    })?;
    file.write_all(bytes)
        .map_err(|error| ShowError::SaveFailed {
            path: destination.to_path_buf(),
            detail: error.to_string(),
        })?;
    file.sync_all().map_err(|error| ShowError::SaveFailed {
        path: destination.to_path_buf(),
        detail: error.to_string(),
    })?;
    fs::rename(temp, destination).map_err(|error| ShowError::SaveFailed {
        path: destination.to_path_buf(),
        detail: error.to_string(),
    })?;
    Ok(())
}

fn temp_path(destination: &Path) -> PathBuf {
    let parent = destination
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let name = destination
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "show".to_string());
    parent.join(format!(".{name}.tmp-{}", std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::show::{BindingAction, Key, KeyChord, OutputDisplay, Show};
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
            let path = std::env::temp_dir().join(format!("dee-nice-show-{tick}-{n}"));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn sample(directory: &Path) -> Show {
        let mut show = Show::new(directory).unwrap();
        show.assign(Key::from_name("b").unwrap(), "b.mp4").unwrap();
        show.assign(Key::from_name("a").unwrap(), "loops/a.mp4")
            .unwrap();
        show.set_muted(true);
        show.set_start_locked(true);
        show.set_output_display(Some(OutputDisplay::new("LG UltraFine", 1).unwrap()));
        show
    }

    fn show_file(directory: &Path) -> PathBuf {
        let path = portable_show_path(directory);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        path
    }

    #[test]
    fn save_and_load_round_trip() {
        let dir = TempDir::new();
        let path = show_file(dir.path());
        let show = sample(dir.path());
        save_show(&path, &show).unwrap();
        let loaded = load_show(&path).unwrap();
        assert_eq!(loaded.media_directory(), dir.path());
        assert_eq!(
            loaded.clip(Key::from_name("a").unwrap()),
            Some("loops/a.mp4")
        );
        assert_eq!(loaded.clip(Key::from_name("b").unwrap()), Some("b.mp4"));
        assert!(loaded.muted());
        assert!(loaded.start_locked());
        assert!(loaded.output_display().is_none());
        let text = fs::read_to_string(&path).unwrap();
        let a = text.find("\"key\": \"a\"").unwrap();
        let b = text.find("\"key\": \"b\"").unwrap();
        assert!(a < b, "assignments are stored by key name");
        assert!(text.ends_with('\n'));
        assert!(text.contains("\"schema_version\": 2"));
        assert!(!text.contains("media_directory"));
        assert!(!text.contains("output_display"));
    }

    #[test]
    fn replacing_a_show_file_keeps_the_new_contents() {
        let dir = TempDir::new();
        let path = show_file(dir.path());
        save_show(&path, &sample(dir.path())).unwrap();
        let mut revised = sample(dir.path());
        revised.set_muted(false);
        revised.clear(Key::from_name("b").unwrap());
        revised.set_output_display(None);
        save_show(&path, &revised).unwrap();
        assert_eq!(load_show(&path).unwrap(), revised);
        let leftovers: Vec<_> = fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(leftovers, vec![std::ffi::OsString::from("show.json")]);
    }

    #[test]
    fn a_failed_replace_leaves_the_previous_file_and_no_temp() {
        let dir = TempDir::new();
        let destination = dir.path().join(".dee-nice").join("show.json");
        fs::create_dir_all(&destination).unwrap();
        let error = save_show(&destination, &sample(dir.path())).unwrap_err();
        assert!(matches!(error, ShowError::SaveFailed { .. }));
        assert!(destination.is_dir());
        let names: Vec<_> = fs::read_dir(destination.parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with('.'))
            .collect();
        assert!(names.is_empty(), "{names:?}");
    }

    #[test]
    fn unsupported_schema_is_rejected_before_unknown_fields() {
        let dir = TempDir::new();
        let path = show_file(dir.path());
        save_show(&path, &sample(dir.path())).unwrap();
        let mut value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        value["schema_version"] = serde_json::json!(3);
        value["future_field"] = serde_json::json!(true);
        fs::write(&path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
        let error = load_show(&path).unwrap_err();
        assert!(matches!(error, ShowError::UnsupportedSchema { found: 3 }));
    }

    #[test]
    fn a_schema_1_show_still_opens_with_its_directory_and_display() {
        let dir = TempDir::new();
        let media = dir.path().join("videos");
        fs::create_dir_all(&media).unwrap();
        let path = dir.path().join("show.dee-nice.json");
        let body = format!(
            r#"{{
  "schema_version": 1,
  "media_directory": "{}",
  "assignments": [{{"key": "a", "clip": "a.mp4"}}],
  "muted": true,
  "start_locked": false,
  "output_display": {{"name": "Studio", "index": 2}}
}}
"#,
            media.display().to_string().replace('\\', "\\\\")
        );
        fs::write(&path, body).unwrap();
        let loaded = load_show(&path).unwrap();
        assert_eq!(loaded.media_directory(), media);
        assert_eq!(loaded.clip(Key::from_name("a").unwrap()), Some("a.mp4"));
        assert!(loaded.muted());
        assert!(!loaded.start_locked());
        assert_eq!(loaded.output_display().unwrap().name(), "Studio");
        assert_eq!(loaded.output_display().unwrap().index(), 2);
    }

    #[test]
    fn malformed_files_and_unknown_fields_are_rejected() {
        let dir = TempDir::new();
        let broken = dir.path().join("broken.dee-nice.json");
        fs::write(&broken, b"{").unwrap();
        assert!(matches!(
            load_show(&broken),
            Err(ShowError::InvalidShowFile { .. })
        ));

        let extra = show_file(dir.path());
        save_show(&extra, &sample(dir.path())).unwrap();
        let mut value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&extra).unwrap()).unwrap();
        value["notes"] = serde_json::json!("operator scratch");
        fs::write(&extra, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
        assert!(matches!(
            load_show(&extra),
            Err(ShowError::InvalidShowFile { .. })
        ));
    }

    #[test]
    fn duplicate_unknown_and_escaping_assignments_fail_the_load() {
        let dir = TempDir::new();
        let path = show_file(dir.path());
        save_show(&path, &sample(dir.path())).unwrap();
        let original = fs::read_to_string(&path).unwrap();

        let cases = [
            original.replace("\"key\": \"b\"", "\"key\": \"a\""),
            original.replace("\"key\": \"b\"", "\"key\": \"ctrl\""),
            original.replace("loops/a.mp4", "../secret.mp4"),
        ];
        for body in cases {
            fs::write(&path, body).unwrap();
            assert!(matches!(
                load_show(&path),
                Err(ShowError::InvalidShowFile { .. })
            ));
        }
    }

    #[test]
    fn a_show_without_a_display_round_trips() {
        let dir = TempDir::new();
        let path = show_file(dir.path());
        let show = Show::new(dir.path()).unwrap();
        save_show(&path, &show).unwrap();
        let loaded = load_show(&path).unwrap();
        assert_eq!(loaded, show);
        assert!(loaded.output_display().is_none());
        assert!(loaded.playback_bindings(false).iter().any(|binding| {
            binding.chord == KeyChord::exit() && binding.action == BindingAction::ExitDisplay
        }));
    }
}
