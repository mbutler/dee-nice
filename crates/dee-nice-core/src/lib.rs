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

//! The show document for dee-nice.
//!
//! This crate owns the media directory, key assignments, mute, the chosen
//! output display, and the bindings that display mode will honor. The portable
//! show file stores the key map, names, black keys, mute, start locked, and
//! auto-lock. The directory is the folder that contains the file, and the
//! display is remembered by the shell.
//! This crate does not open windows or start a video player.

mod error;
mod persist;
mod show;

pub use error::ShowError;
pub use persist::{SHOW_SCHEMA_VERSION, load_show, portable_show_path, save_show};
pub use show::{
    AUTO_LOCK_SECONDS, Assignment, BindingAction, Key, KeyChord, OutputDisplay, PlaybackBinding,
    Show, Slot, Template,
};
