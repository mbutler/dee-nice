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

//! Setup window and mpv display mode for dee-nice.
//!
//! The core still owns the show. This crate scans folders, lists displays,
//! and supervises mpv.

mod app;
mod derivatives;
mod displays;
mod layout;
mod media;
mod playback;
mod readiness;
mod session;
mod store;
mod theme;
mod thumbs;
mod tools;

pub use app::run;
