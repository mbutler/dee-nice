use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use dee_nice_core::{OutputDisplay, Show, load_show, portable_show_path, save_show};
use serde::{Deserialize, Serialize};

pub struct Startup {
    pub show: Show,
    pub show_path: PathBuf,
    pub save_enabled: bool,
    pub machine_writable: bool,
    pub notice: Option<String>,
    pub machine: Machine,
}

#[derive(Debug)]
pub struct Opened {
    pub show: Show,
    pub show_path: PathBuf,
    pub machine: Machine,
}

#[derive(Clone, Debug, Default)]
pub struct Machine {
    last_folder: Option<PathBuf>,
    displays: BTreeMap<String, SavedDisplay>,
    presentation: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SavedDisplay {
    name: String,
    index: u32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MachineFile {
    schema_version: u32,
    last_folder: Option<String>,
    displays: BTreeMap<String, SavedDisplay>,
    /// Absent on older files. Those Macs stay in kiosk mode.
    #[serde(default)]
    presentation: bool,
}

const MACHINE_SCHEMA: u32 = 1;

pub fn default_support_dir() -> PathBuf {
    if cfg!(target_os = "macos")
        && let Some(home) = std::env::var_os("HOME")
    {
        return PathBuf::from(home).join("Library/Application Support/dee-nice");
    }
    if let Some(base) = std::env::var_os("XDG_DATA_HOME") {
        return PathBuf::from(base).join("dee-nice");
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(".local/share/dee-nice");
    }
    std::env::temp_dir().join("dee-nice")
}

pub fn legacy_show_path(support_dir: &Path) -> PathBuf {
    support_dir.join("show.dee-nice.json")
}

fn machine_path(support_dir: &Path) -> PathBuf {
    support_dir.join("machine.json")
}

impl Machine {
    pub fn display_for(&self, folder: &Path) -> Option<OutputDisplay> {
        let key = folder.to_str()?;
        let saved = self.displays.get(key)?;
        OutputDisplay::new(saved.name.clone(), saved.index).ok()
    }

    pub fn set_last_folder(&mut self, folder: &Path) {
        self.last_folder = Some(folder.to_path_buf());
    }

    pub fn set_display(&mut self, folder: &Path, display: Option<&OutputDisplay>) {
        let Some(key) = folder.to_str() else {
            return;
        };
        match display {
            Some(display) => {
                self.displays.insert(
                    key.to_string(),
                    SavedDisplay {
                        name: display.name().to_string(),
                        index: display.index(),
                    },
                );
            }
            None => {
                self.displays.remove(key);
            }
        }
    }

    pub fn presentation(&self) -> bool {
        self.presentation
    }

    pub fn set_presentation(&mut self, presentation: bool) {
        self.presentation = presentation;
    }
}

pub fn load_startup(support_dir: &Path) -> Startup {
    let pending = support_dir.join("pending");
    let machine_file = machine_path(support_dir);
    let (machine, machine_writable, machine_notice) = match load_machine(&machine_file) {
        Ok(machine) => (machine, true, None),
        Err(error) if machine_file.exists() => (
            Machine::default(),
            false,
            Some(format!(
                "{error} This Mac's display memory was left untouched."
            )),
        ),
        Err(_) => (Machine::default(), true, None),
    };
    if let Some(notice) = machine_notice {
        return blank(pending, machine, false, machine_writable, Some(notice));
    }

    if let Some(folder) = machine.last_folder.clone() {
        let path = portable_show_path(&folder);
        if path.exists() {
            return startup_from_portable(&folder, &path, machine, machine_writable);
        }
        let mut show = match Show::new(&folder) {
            Ok(show) => show,
            Err(error) => {
                return blank(
                    pending,
                    machine,
                    false,
                    machine_writable,
                    Some(error.to_string()),
                );
            }
        };
        if let Some(display) = machine.display_for(&folder) {
            show.set_output_display(Some(display));
        }
        return Startup {
            show,
            show_path: path,
            save_enabled: true,
            machine_writable,
            notice: None,
            machine,
        };
    }

    let legacy = legacy_show_path(support_dir);
    if legacy.exists() {
        return migrate_legacy(support_dir, &legacy, machine_writable);
    }

    blank(pending, machine, false, machine_writable, None)
}

pub fn open_folder(folder: &Path, machine: &Machine) -> Result<Opened, String> {
    let folder = folder
        .canonicalize()
        .map_err(|error| format!("could not open {}: {error}", folder.display()))?;
    let path = portable_show_path(&folder);
    let mut show = if path.exists() {
        load_show(&path)
            .map_err(|error| format!("{error} The folder's show file was left untouched."))?
    } else {
        Show::new(&folder).map_err(|error| error.to_string())?
    };
    let mut machine = machine.clone();
    machine.set_last_folder(&folder);
    if let Some(display) = machine.display_for(&folder) {
        show.set_output_display(Some(display));
    }
    Ok(Opened {
        show,
        show_path: path,
        machine,
    })
}

pub fn save(path: &Path, show: &Show) -> Result<(), String> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
    }
    save_show(path, show).map_err(|error| error.to_string())
}

pub fn save_machine(support_dir: &Path, machine: &Machine) -> Result<(), String> {
    fs::create_dir_all(support_dir)
        .map_err(|error| format!("could not create {}: {error}", support_dir.display()))?;
    let last_folder = match &machine.last_folder {
        Some(folder) => Some(
            folder
                .to_str()
                .ok_or_else(|| "the folder path is not valid Unicode".to_string())?
                .to_string(),
        ),
        None => None,
    };
    let file = MachineFile {
        schema_version: MACHINE_SCHEMA,
        last_folder,
        displays: machine.displays.clone(),
        presentation: machine.presentation,
    };
    let mut bytes = serde_json::to_vec_pretty(&file).map_err(|error| error.to_string())?;
    bytes.push(b'\n');
    atomic_write(&machine_path(support_dir), &bytes)
}

fn startup_from_portable(
    folder: &Path,
    path: &Path,
    machine: Machine,
    machine_writable: bool,
) -> Startup {
    match load_show(path) {
        Ok(mut show) => {
            if let Some(display) = machine.display_for(folder) {
                show.set_output_display(Some(display));
            }
            Startup {
                show,
                show_path: path.to_path_buf(),
                save_enabled: true,
                machine_writable,
                notice: None,
                machine,
            }
        }
        Err(error) => Startup {
            show: Show::new(folder).unwrap_or_else(|_| empty_show()),
            show_path: path.to_path_buf(),
            save_enabled: false,
            machine_writable,
            notice: Some(format!(
                "{error} Changes stay in memory until you save over this file."
            )),
            machine,
        },
    }
}

fn migrate_legacy(support_dir: &Path, legacy: &Path, machine_writable: bool) -> Startup {
    let legacy_bytes = fs::read(legacy).unwrap_or_default();
    let loaded = match load_show(legacy) {
        Ok(show) => show,
        Err(error) => {
            return blank(
                support_dir.join("pending"),
                Machine::default(),
                false,
                machine_writable,
                Some(format!(
                    "{error} The previous show file was left untouched."
                )),
            );
        }
    };
    let folder = loaded.media_directory().to_path_buf();
    let path = portable_show_path(&folder);
    if !path.exists()
        && let Err(error) = save(&path, &loaded)
    {
        return Startup {
            show: loaded,
            show_path: path,
            save_enabled: false,
            machine_writable,
            notice: Some(format!(
                "{error} The previous show file was left untouched."
            )),
            machine: Machine::default(),
        };
    }
    let mut machine = Machine::default();
    machine.set_last_folder(&folder);
    if let Some(display) = loaded.output_display() {
        machine.set_display(&folder, Some(display));
    }
    let mut show = match load_show(&path) {
        Ok(show) => show,
        Err(error) => {
            return Startup {
                show: loaded,
                show_path: path,
                save_enabled: false,
                machine_writable: false,
                notice: Some(error.to_string()),
                machine,
            };
        }
    };
    if let Some(display) = machine.display_for(&folder) {
        show.set_output_display(Some(display));
    }
    let notice = if machine_writable {
        save_machine(support_dir, &machine).err()
    } else {
        None
    };
    if fs::read(legacy).ok().as_deref() != Some(legacy_bytes.as_slice()) {
        return Startup {
            show,
            show_path: path,
            save_enabled: false,
            machine_writable: false,
            notice: Some("The previous show file changed while it was being copied.".to_string()),
            machine,
        };
    }
    Startup {
        show,
        show_path: path,
        save_enabled: true,
        machine_writable,
        notice,
        machine,
    }
}

fn load_machine(path: &Path) -> Result<Machine, String> {
    if !path.exists() {
        return Ok(Machine::default());
    }
    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("machine file is invalid: {error}"))?;
    let schema = value
        .get("schema_version")
        .and_then(|version| version.as_u64())
        .ok_or_else(|| "machine file is invalid: schema_version is missing".to_string())?;
    if schema != u64::from(MACHINE_SCHEMA) {
        return Err(format!(
            "machine file uses schema version {schema}, and this app reads schema {MACHINE_SCHEMA}"
        ));
    }
    let file: MachineFile = serde_json::from_value(value)
        .map_err(|error| format!("machine file is invalid: {error}"))?;
    let last_folder = file.last_folder.map(PathBuf::from);
    Ok(Machine {
        last_folder,
        displays: file.displays,
        presentation: file.presentation,
    })
}

fn blank(
    pending: PathBuf,
    machine: Machine,
    save_enabled: bool,
    machine_writable: bool,
    notice: Option<String>,
) -> Startup {
    Startup {
        show: Show::new(&pending).unwrap_or_else(|_| empty_show()),
        show_path: portable_show_path(&pending),
        save_enabled,
        machine_writable,
        notice,
        machine,
    }
}

fn empty_show() -> Show {
    Show::new(".").unwrap_or_else(|_| unreachable!("a dot path is a valid media directory"))
}

fn atomic_write(destination: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = destination
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let name = destination
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "machine.json".to_string());
    let temp = parent.join(format!(".{name}.tmp-{}", std::process::id()));
    let result = (|| {
        let mut file = File::create(&temp).map_err(|error| error.to_string())?;
        file.write_all(bytes).map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
        fs::rename(&temp, destination).map_err(|error| error.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use dee_nice_core::Key;
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
            let path = std::env::temp_dir().join(format!("dee-nice-store-{tick}-{n}"));
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
    fn a_fresh_mac_waits_for_a_folder() {
        let support = TempDir::new();
        let startup = load_startup(&support.0);
        assert!(!startup.save_enabled);
        assert!(startup.notice.is_none());
        assert!(!startup.show_path.exists());
        assert!(startup.machine_writable);
    }

    #[test]
    fn an_old_show_file_is_copied_into_the_video_folder_once() {
        let support = TempDir::new();
        let media = TempDir::new();
        fs::write(media.0.join("a.mp4"), b"video").unwrap();
        let legacy = legacy_show_path(&support.0);
        fs::create_dir_all(&support.0).unwrap();
        let body = format!(
            r#"{{
  "schema_version": 1,
  "media_directory": "{}",
  "assignments": [{{"key": "a", "clip": "a.mp4"}}],
  "muted": false,
  "start_locked": true,
  "output_display": {{"name": "Studio", "index": 1}}
}}
"#,
            media.0.display().to_string().replace('\\', "\\\\")
        );
        fs::write(&legacy, &body).unwrap();

        let startup = load_startup(&support.0);
        assert!(startup.save_enabled, "{:?}", startup.notice);
        assert_eq!(startup.show.media_directory(), media.0);
        assert_eq!(
            startup.show.clip(Key::from_name("a").unwrap()),
            Some("a.mp4")
        );
        assert!(startup.show.start_locked());
        assert_eq!(startup.show.output_display().unwrap().name(), "Studio");
        assert_eq!(fs::read_to_string(&legacy).unwrap(), body);
        let portable = fs::read_to_string(&startup.show_path).unwrap();
        assert!(portable.contains("\"schema_version\": 2"));
        assert!(!portable.contains("Studio"));
        assert!(!portable.contains("media_directory"));

        let again = load_startup(&support.0);
        assert_eq!(again.show.clip(Key::from_name("a").unwrap()), Some("a.mp4"));
        assert_eq!(again.show.output_display().unwrap().name(), "Studio");
        assert_eq!(fs::read_to_string(&legacy).unwrap(), body);
    }

    #[test]
    fn a_broken_old_show_file_is_left_in_place() {
        let support = TempDir::new();
        let legacy = legacy_show_path(&support.0);
        fs::create_dir_all(&support.0).unwrap();
        fs::write(&legacy, b"{").unwrap();
        let startup = load_startup(&support.0);
        assert!(!startup.save_enabled);
        assert!(startup.notice.unwrap().contains("invalid"));
        assert_eq!(fs::read(&legacy).unwrap(), b"{");
        assert!(!portable_show_path(startup.show.media_directory()).exists());
    }

    #[test]
    fn opening_another_folder_uses_that_folders_show() {
        let media = TempDir::new();
        let other = TempDir::new();
        let mut first = Show::new(&media.0).unwrap();
        first.assign(Key::from_name("a").unwrap(), "a.mp4").unwrap();
        save(&portable_show_path(&media.0), &first).unwrap();
        let mut second = Show::new(&other.0).unwrap();
        second
            .assign(Key::from_name("b").unwrap(), "b.mp4")
            .unwrap();
        save(&portable_show_path(&other.0), &second).unwrap();

        let other_folder = other.0.canonicalize().unwrap();
        let mut machine = Machine::default();
        machine.set_last_folder(&media.0);
        machine.set_display(&other_folder, Some(&OutputDisplay::new("Hall", 3).unwrap()));
        let opened = open_folder(&other.0, &machine).unwrap();
        assert_eq!(
            opened.show.clip(Key::from_name("b").unwrap()),
            Some("b.mp4")
        );
        assert!(opened.show.clip(Key::from_name("a").unwrap()).is_none());
        assert_eq!(opened.show.output_display().unwrap().name(), "Hall");
        assert!(!fs::read(portable_show_path(&media.0)).unwrap().is_empty());
    }

    #[test]
    fn a_broken_folder_show_does_not_replace_the_current_one() {
        let media = TempDir::new();
        let path = portable_show_path(&media.0);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"{").unwrap();
        let error = open_folder(&media.0, &Machine::default()).unwrap_err();
        assert!(error.contains("invalid"));
        assert_eq!(fs::read(&path).unwrap(), b"{");
    }

    #[test]
    fn presentation_is_off_until_this_mac_turns_it_on() {
        let support = TempDir::new();
        fs::create_dir_all(&support.0).unwrap();
        fs::write(
            support.0.join("machine.json"),
            r#"{"schema_version":1,"last_folder":null,"displays":{}}"#,
        )
        .unwrap();
        let startup = load_startup(&support.0);
        assert!(!startup.machine.presentation());

        let mut machine = startup.machine;
        machine.set_presentation(true);
        save_machine(&support.0, &machine).unwrap();
        let again = load_startup(&support.0);
        assert!(again.machine.presentation());
    }

    #[test]
    fn the_support_directory_is_named_dee_nice() {
        let path = default_support_dir();
        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some("dee-nice")
        );
    }
}
