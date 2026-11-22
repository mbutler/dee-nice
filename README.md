# dee-nice

dee-nice is an updated Rave Kontrol. It plays a folder of videos from the keyboard for kiosks, galleries, installations, and a simple show-control desk.

The person at the keyboard is the operator. Visitors are not. Display mode keeps playing if someone mashes keys, and the operator can always get back to setup.

## Install

Download [dee-nice 0.1.0](https://github.com/mbutler/dee-nice/releases/tag/v0.1.0). Open `dee-nice.dmg` and drag **dee-nice** to Applications. There is no installer, and there is nothing else to install. mpv and ffmpeg are inside the app.

The app lives in Applications. The videos live in their own folder. Do not put the app inside the video folder.

This build is ad-hoc signed and not notarized. Gatekeeper on another Mac will block the downloaded app until it is signed with a Developer ID.

## Run a show

1. Choose a folder of videos.
2. Drag a thumbnail onto a letter or digit, or select a thumbnail and click the key. The same clip may sit on more than one key. Black is the first card. Drag it onto a key to cut that slot to black.
3. Choose the output display. A Mac with one connected display selects it. Mute is a checkbox. Sound is on until the show is muted.
4. Enter display mode. A key plays its clip. The clip loops. Another key hard-cuts. Ctrl+Shift+L locks and unlocks clip keys. While locked, the current picture keeps looping and clip keys do nothing. Ctrl+Shift+K leaves display mode, including while locked, and returns to setup.

Enter display mode stays off until a folder, a connected display, and at least one key are set. The reason sits beside the button. A missing assigned file is reported there and is not deleted from the show.

Click an assigned key to select it, then type in the Name field. A name is at most 24 characters and is drawn on the key. The same clip on two keys can have two names. Replacing the clip clears the name. Assigning that same clip again keeps it. An empty name clears it. Right-click a key to clear the assignment. Clicking the same assignment again selects it for naming.

Start locked plays the first assignment in each slot when display mode opens, in key order: letters, digits, function keys, space, then arrows. That assignment can be black. Clip keys do nothing until Ctrl+Shift+L. Without Start locked, display mode opens on black and waits for a key.

Auto-lock is off until the show turns it on. The checkbox uses 60 seconds. After that long with no key, clip keys lock. They do not toggle. A play or a lock starts the wait again. Unlock starts it again. Locking when the show is already locked does nothing.

Display mode keeps this Mac awake. The screen stays on, and the Mac does not idle-sleep, until the show ends. Setup does not hold the machine awake. There is no checkbox for this.

## Layouts

The layout is one choice for the whole show. Full is the default: one picture on the chosen display.

Side by side, Stacked, Four, and Portraits divide that same display. Select a slot, then assign keys to it. A key hard-cuts only its slot. The other slots keep looping. An empty slot stays black. A divided key shows a small slot mark.

Portraits is two tall frames, centered, with black around them. There is no layout editor and no way to rotate a clip.

The layout cannot be changed while a show is playing. Changing it when a slot disappears moves those keys onto the first slot and keeps the clip and the name.

## Kiosk and presentation

Kiosk is the default, and it is stored on this Mac, not in the show. Display mode hides the setup window. Copying a video folder onto another Mac does not turn presentation on.

Presentation, with two or more displays, leaves the setup window on the other display. It prefers the primary display when that is not the output. The live key is marked. Lock keys and Leave display mode stay on that window. The chords still work. The window is placed once per show, so it can be dragged afterward. Turning Presentation on or off during a show places it again.

With only one display, Presentation still hides the window.

## What is saved

The video folder is the thing to copy.

| What | Where | Copies with the videos |
| --- | --- | --- |
| The clips | The folder you chose | Yes |
| The show | `<folder>/.dee-nice/show.json` | Yes. Finder hides `.dee-nice`. |
| The last folder, this Mac's display, and kiosk or presentation | `~/Library/Application Support/dee-nice/machine.json` | No |
| Lighter copies and thumbnails | `~/Library/Application Support/dee-nice/` | No |

The show stores the layout, the keys, each key's slot, names, black keys, mute, start locked, and auto-lock. It does not store the display. The other Mac picks its own screen.

A failed save leaves the previous show file untouched. A show file that fails to load is left untouched until **Save over the show file**. That does not replace a show that is already open and valid.

An older `show.dee-nice.json` in Application Support is copied into its video folder once and left in place.

## Lighter copies

Assigned clips are checked against this Mac and the chosen display. H.264 or HEVC whose long edge fits plays as the original. Anything larger, or another codec, gets one H.264 copy under Application Support. The source is only read. Nothing is written beside the originals.

The fit is the smaller of the display's pixel long edge and this Mac's cap. The cap is 1920 when Low Power Mode is on, the Mac is Intel, or memory is 8 GB or less. Otherwise the Apple Silicon cap is 3840. The picture is never scaled up. The setup window says whether this Mac prepares copies up to 1080p or up to 4K.

The first key on a file that does not fit starts that copy. A second key does not start another. Clearing the last key cancels it and deletes the copy. Unassigned files are never transcoded. Enter display mode does not wait. If the copy is not ready, that key plays the source once. The next press plays the copy. The file already playing is not cut over mid-play. If encoding fails, the error is shown and the source plays.

A half-screen slot still uses the full-display fit. Thumbnails are separate, one at a time, in `Thumbnails`. A missing ffmpeg leaves the names and keys usable, without pictures.

Details are in `docs/derivatives.md`.

## What this version does not do

Crossfades, layers, mixing, MIDI, and a pinned derivative size are not in this version. The engine, the hard cut, and the two chords are fixed. They are not settings. macOS is the system this is checked on.

## Build

The repo pins Rust 1.100.0 in `rust-toolchain.toml`, including rustfmt and clippy.

```bash
cargo test --workspace --locked
cargo run -p dee-nice
```

`scripts/bundle-macos.sh` builds `dist/dee-nice.app` from the official mpv.app in `.tools/mpv-macos` and the conda-forge ffmpeg in `.tools/mpv/bin`, then writes `dist/dee-nice.dmg`. A build of mpv with the windowing backend disabled cannot be the bundled player.

For development without the bundle, set `DEE_NICE_MPV` to an mpv that can open a window, or install `mpv` on `PATH`. Set `DEE_NICE_FFMPEG` and `DEE_NICE_FFPROBE` the same way, or leave ffmpeg on `PATH`. Without ffmpeg, assigned clips play as the original files. The ordinary tests do not need either tool. `display_mode_switches_locks_and_quits` is ignored until `DEE_NICE_MPV` is set.

## License

Copyright (C) 2026 Matthew Butler.

dee-nice is free software, licensed under the GNU General Public License, version 3 or any later version. The license text is in `LICENSE`.

mpv and ffmpeg stay separate programs inside the app. mpv is GPL-2.0-or-later. The bundled ffmpeg build is GPL-3.0-or-later because it is configured with `--enable-gpl`, `--enable-version3`, and libx264. Their notices are in `THIRD-PARTY-NOTICES`. Distributing the app means conveying those programs under their licenses too, including their source.
