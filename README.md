# dee-nice

An updated Rave Kontrol. dee-nice plays a folder of videos from the keyboard for kiosks, galleries, installations, and simple show control.

The operator drags a video thumbnail onto a letter or digit, chooses the output monitor, and enters display mode. A key plays that clip fullscreen. Another key hard-cuts to its clip. Clips loop. Ctrl+Shift+L locks and unlocks key switching. Ctrl+Shift+K leaves display mode. Sound plays unless the show is muted.

The show is stored in the video folder at `.dee-nice/show.json`. This Mac remembers the display separately. Assigned clips that do not fit this Mac get a lighter H.264 copy in Application Support, as described in `docs/derivatives.md`.

## Build

The repo pins Rust 1.100.0 in `rust-toolchain.toml`, including rustfmt and clippy.

```bash
cargo test --workspace --locked
cargo run -p dee-nice
```

A person using the finished app does not install mpv or ffmpeg. Those ship inside `dee-nice.app`. On this Mac, `scripts/bundle-macos.sh` builds `dist/dee-nice.app` from the official mpv.app in `.tools/mpv-macos` and the conda-forge ffmpeg in `.tools/mpv/bin`. That bundle is a local proof. It is not notarized, and it is not ready to hand to anyone until a license is chosen.

For development without the bundle, set `DEE_NICE_MPV` to an mpv that can open a window, or install `mpv` on `PATH`. Set `DEE_NICE_FFMPEG` and `DEE_NICE_FFPROBE` the same way, or leave ffmpeg on `PATH`. Without ffmpeg, assigned clips play as the original files. The ordinary tests do not need either tool. `display_mode_switches_locks_and_quits` is ignored until `DEE_NICE_MPV` is set.

## Limitations

- macOS is the system this is checked on. Windows and Linux are not verified.
- `dist/dee-nice.app` runs locally. It is ad-hoc signed, not notarized. A build of mpv with Cocoa disabled cannot be the bundled player.
- A license has not been chosen. mpv and ffmpeg bring their own licenses when they are bundled.
