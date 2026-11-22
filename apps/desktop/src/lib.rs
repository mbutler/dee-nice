//! Setup window and mpv display mode for dee-nice.
//!
//! The core still owns the show. This crate scans folders, lists displays,
//! and supervises mpv.

mod app;
mod derivatives;
mod displays;
mod media;
mod playback;
mod readiness;
mod session;
mod store;
mod theme;
mod thumbs;
mod tools;

pub use app::run;
