use std::path::PathBuf;

/// A failure while editing, saving, or loading a show.
#[derive(Debug)]
pub enum ShowError {
    EmptyMediaDirectory,
    InvalidClipPath { path: String, reason: &'static str },
    EmptyDisplayName,
    UnsupportedSchema { found: u32 },
    ShowFileTooLarge { path: PathBuf, bytes: u64 },
    InvalidShowFile { path: PathBuf, detail: String },
    PathNotUnicode { role: &'static str },
    SaveFailed { path: PathBuf, detail: String },
    LoadFailed { path: PathBuf, detail: String },
}

impl std::fmt::Display for ShowError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyMediaDirectory => write!(formatter, "the media directory is empty"),
            Self::InvalidClipPath { path, reason } => {
                write!(formatter, "clip path \"{path}\" is invalid: {reason}")
            }
            Self::EmptyDisplayName => write!(formatter, "the output display name is empty"),
            Self::UnsupportedSchema { found } => write!(
                formatter,
                "show file uses schema version {found}, and this app reads schema 1 and schema {supported}",
                supported = crate::SHOW_SCHEMA_VERSION
            ),
            Self::ShowFileTooLarge { path, bytes } => write!(
                formatter,
                "show file {} is {bytes} bytes, above the 1 MiB limit",
                path.display()
            ),
            Self::InvalidShowFile { path, detail } => {
                write!(
                    formatter,
                    "show file {} is invalid: {detail}",
                    path.display()
                )
            }
            Self::PathNotUnicode { role } => {
                write!(formatter, "the {role} path is not valid Unicode")
            }
            Self::SaveFailed { path, detail } => {
                write!(formatter, "could not save {}: {detail}", path.display())
            }
            Self::LoadFailed { path, detail } => {
                write!(formatter, "could not load {}: {detail}", path.display())
            }
        }
    }
}

impl std::error::Error for ShowError {}
