//! The show document for dee-nice.
//!
//! This crate owns the media directory, key assignments, mute, the chosen
//! output display, and the bindings that display mode will honor. The portable
//! show file stores the key map, mute, and start locked. The directory is the
//! folder that contains the file, and the display is remembered by the shell.
//! This crate does not open windows or start a video player.

mod error;
mod persist;
mod show;

pub use error::ShowError;
pub use persist::{SHOW_SCHEMA_VERSION, load_show, portable_show_path, save_show};
pub use show::{BindingAction, Key, KeyChord, OutputDisplay, PlaybackBinding, Show};
