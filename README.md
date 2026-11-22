# dee-nice

An updated Rave Kontrol. dee-nice plays a folder of videos from the keyboard for kiosks, galleries, installations, and simple show control.

The operator drags a video thumbnail onto a letter or digit, chooses the output monitor, and enters display mode. A key plays that clip fullscreen. Another key hard-cuts to its clip. Clips loop. Ctrl+Shift+L locks and unlocks key switching. Ctrl+Shift+K leaves display mode. Sound plays unless the show is muted.

The show is stored in the video folder at `.dee-nice/show.json`. The screen can stay one picture or be split into a fixed layout. A key can have a short name, play into one slot, or cut that slot to black. Auto-lock is off unless that show turns it on. This Mac remembers the display separately. Assigned clips that do not fit this Mac get a lighter H.264 copy in Application Support, as described in `docs/derivatives.md`.

## Build

The repo pins Rust 1.100.0 in `rust-toolchain.toml`, including rustfmt and clippy.

```bash
cargo test --workspace --locked
cargo run -p dee-nice
```

A person using the finished app does not install mpv or ffmpeg. Those ship inside `dee-nice.app`. On this Mac, `scripts/bundle-macos.sh` builds `dist/dee-nice.app` from the official mpv.app in `.tools/mpv-macos` and the conda-forge ffmpeg in `.tools/mpv/bin`, then writes `dist/dee-nice.dmg`. That disk image is the single file a person downloads. It is not notarized.

For development without the bundle, set `DEE_NICE_MPV` to an mpv that can open a window, or install `mpv` on `PATH`. Set `DEE_NICE_FFMPEG` and `DEE_NICE_FFPROBE` the same way, or leave ffmpeg on `PATH`. Without ffmpeg, assigned clips play as the original files. The ordinary tests do not need either tool. `display_mode_switches_locks_and_quits` is ignored until `DEE_NICE_MPV` is set.

## Limitations

- macOS is the system this is checked on. Windows and Linux are not verified.
- `dist/dee-nice.app` runs locally. It is ad-hoc signed, not notarized. A build of mpv with Cocoa disabled cannot be the bundled player.

## License

Copyright (C) 2026 Matthew Butler.

dee-nice is free software, licensed under the GNU General Public License, version 3 or any later version. The license text is in `LICENSE`.

mpv and ffmpeg stay separate programs inside the app. mpv is GPL-2.0-or-later. The bundled ffmpeg build is GPL-3.0-or-later because it is configured with `--enable-gpl`, `--enable-version3`, and libx264. Their notices are in `THIRD-PARTY-NOTICES`. Distributing the app means conveying those programs under their licenses too, including their source.
