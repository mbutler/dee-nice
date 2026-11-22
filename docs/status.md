# Current status

Current milestone: assign clips to keys, choose a display, and play one looping clip fullscreen with hard cuts, lock, and exit.

Active task: none. Tasks 001 and 002 are done. The show file lives in the video folder, assigned clips that do not fit this Mac get a lighter copy outside that folder, and `scripts/bundle-macos.sh` builds one `dee-nice.app`. The next slice is a license and notarization before that app is handed to anyone.

Checkpoint: uncommitted workspace on `main`. No commit yet.

## Working behavior

The setup window shows video thumbnails and a keyboard of letters and digits. Drag a thumbnail onto a key, or select one and click the key. The key shows the thumbnail. Function keys, space, and arrows are not on the grid; a show file that already uses them still opens and those keys still play. The window remembers mute, start-locked, and the output display. This Mac defaults to kiosk: display mode hides the window. Presentation, stored in `machine.json`, keeps the window on another display and marks the live key. One display still hides the window. Enter display mode supervises mpv. A key hard-cuts to its clip and the clip loops. Ctrl+Shift+L locks and unlocks clip keys. Ctrl+Shift+K quits mpv and returns to setup. An unlocked session starts on black. A locked start plays the first assigned clip in key order (letters, then digits, function keys, space, then arrows).

The show file is `<folder>/.dee-nice/show.json`. It stores the key map, mute, and start locked. This Mac stores the last folder and that folder's display in `~/Library/Application Support/dee-nice/machine.json`. An older `show.dee-nice.json` in Application Support is copied into its video folder once and left in place. A display that is not connected is forgotten, and a Mac with one display selects it. Edits save when the folder's show file loaded cleanly. A file that fails to load is left untouched until the operator clicks "Save over the show file".

Clip keys live in an mpv input section named `dee-nice-clips`. Lock disables that section. Rewriting `input.conf` does not remove keys mpv already loaded, so the session does not use `load-input-conf` for the lock.

Thumbnails are made one at a time with ffmpeg and stored in `~/Library/Application Support/dee-nice/Thumbnails/`. A missing ffmpeg leaves the names and keys usable, without pictures. Assigned clips are probed against this Mac and the chosen display. H.264 or HEVC whose long edge fits plays as the original. Anything larger, or another codec, gets one H.264 copy under `~/Library/Application Support/dee-nice/Derivatives/`. The fit is 1080p when Low Power Mode is on, the Mac is Intel, or memory is 8 GB or less, and 4K otherwise, never larger than the display. The first key on a file starts that copy. A second key does not start another. Clearing the last key cancels it. Enter display mode does not wait. The next press of that key uses the copy once it exists. A missing ffmpeg is reported and the original plays.

## Verification

On 22 November 2026, macOS arm64, Rust 1.100.0:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

`cargo test` ran 17 core tests and 27 desktop tests, with 2 desktop tests ignored. With `DEE_NICE_MPV` pointed at the official mpv 0.42.0 macOS 27 build, `display_mode_switches_locks_and_quits` passed earlier the same day: fullscreen was true, A loaded `red.mp4`, B cut to `blue.mp4`, Ctrl+Shift+L left B playing when A was pressed again, and Ctrl+Shift+K returned "Left display mode." That ignored test was not part of the later locked run.

The conda-forge ffmpeg at `.tools/mpv/bin/ffmpeg` encoded `/tmp/dee-nice-demo/red.mp4` with `h264_videotoolbox` to a 320×180 H.264 file, and wrote a 240×136 PNG thumbnail with the same seek and scale the setup window uses.

`scripts/bundle-macos.sh` then built `dist/dee-nice.app`. `codesign --verify --deep --strict` accepted it. With `PATH` limited to `/usr/bin:/bin` and the `DEE_NICE_*` overrides unset, the app reported the nested official mpv and the bundled ffmpeg and ffprobe. That ffmpeg wrote another 240×136 PNG, and `mpv --version` printed 0.42.0. Clearing the quarantine attribute on `libMoltenVK.dylib` was denied; the signature check and `mpv --version` still succeeded.

The operator opened the setup window on 22 November 2026, pointed it at a folder of videos, and confirmed the screen works. An earlier build painted an egui id clash across the video list and the key grid; those scroll areas now have separate ids.

## Remaining work

- Engine, fade, and the two chords are fixed. They are not settings. A pinned derivative size is also deferred.
- `dist/dee-nice.app` is ad-hoc signed and not notarized. The ffmpeg inside it is the local conda-forge build. The nested player is the official mpv 0.42.0 macOS app. The conda-forge mpv in `.tools/mpv` is built with Cocoa disabled and is not copied into the bundle. `cargo run` still uses `DEE_NICE_MPV`, `DEE_NICE_FFMPEG`, `DEE_NICE_FFPROBE`, or `PATH`.
- Windows and Linux are not verified.
- No license has been chosen.

## Next action

Choose a license, then notarize `dee-nice.app`, before handing it to anyone. Do not add settings for the fixed MVP choices.
