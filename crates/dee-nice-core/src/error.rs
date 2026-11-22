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

use std::path::PathBuf;

/// A failure while editing, saving, or loading a show.
#[derive(Debug)]
pub enum ShowError {
    EmptyMediaDirectory,
    InvalidClipPath { path: String, reason: &'static str },
    UnassignedKey,
    InvalidKeyName { reason: &'static str },
    InvalidAutoLock,
    SlotNotInTemplate,
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
            Self::UnassignedKey => write!(formatter, "that key has no assignment"),
            Self::InvalidKeyName { reason } => {
                write!(formatter, "the key name is invalid: {reason}")
            }
            Self::InvalidAutoLock => {
                write!(formatter, "auto-lock must be between 1 second and 24 hours")
            }
            Self::SlotNotInTemplate => {
                write!(formatter, "that slot is not part of the current layout")
            }
            Self::EmptyDisplayName => write!(formatter, "the output display name is empty"),
            Self::UnsupportedSchema { found } => write!(
                formatter,
                "show file uses schema version {found}, and this app reads schemas 1 through {supported}",
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
