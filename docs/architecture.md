# Architecture

dee-nice is a workspace with a headless core and a desktop shell. The core owns the show. The shell owns the setup window, the monitor list, and the mpv process. The core does not depend on a GUI toolkit or on mpv.

```mermaid
flowchart TD
    UI["Setup window"] --> Engine["Show operations"]
    UI --> Playback["mpv supervisor"]
    Playback --> Engine
    Engine --> Model["Show, keys, bindings"]
    Persist["Show file"] --> Model
    Playback --> Platform["mpv process and displays"]
```

## Crates

| Path | Owns |
| --- | --- |
| `crates/dee-nice-core` | The show, validation, playback bindings, and the versioned show file |
| `apps/desktop` | The setup window, the display list, and the mpv supervisor |

`unsafe` is forbidden in both crates. Playback stays outside the core so that can remain true: the shell supervises the `mpv` binary over its IPC socket and a generated input map. libmpv would pull FFI and unsafe into the app for no gain in this version. mpv already hardware-decodes, loops, fullscreens onto a chosen screen, and hard-cuts with `loadfile replace`.

The setup window is egui in `apps/desktop`. It lists the folder, edits the show through the core API, and starts display mode. See `docs/ui.md`.

Lock and exit are in the mpv input file from the start, so Ctrl+Shift+K still quits if IPC never connects. Clip keys are sent with `define-section dee-nice-clips … force` and removed with `disable-section`. `load-input-conf` leaves keys that were already loaded, so the session does not use it to lock. The section needs the `force` flag; without it mpv accepts the section and then ignores its keys. The shell passes `--screen-name` and `--fs-screen-name` only. A numeric screen index would override the name, and mpv's index is not the index stored on the show.

## Show

`Show` is the document. Fields are private to callers outside the core. Edits go through methods, and a rejected edit leaves the previous show in place.

| Operation | Behavior |
| --- | --- |
| `assign` / `clear` | One relative clip path per key. Absolute paths and `..` are rejected. |
| `set_muted` | Durable mute flag. The shell applies it when display mode starts. |
| `set_start_locked` | The lock copied into the session when display mode starts. |
| `set_output_display` | Monitor name plus index, kept in memory for the session. Empty names are rejected. The shell stores it in `machine.json`, not in the show file. |
| `playback_bindings(locked)` | Play bindings for each assignment, then Ctrl+Shift+L and Ctrl+Shift+K. A locked session omits the play bindings and keeps both chords. |

`start_locked` is not the live lock. The shell holds the session lock. Clip keys always come from the unlocked bindings, and `start_locked` decides whether that section starts enabled.

The setup keyboard offers `a`–`z` and `0`–`9`, in keyboard order. The show file still accepts `f1`–`f12`, `space`, and the arrow keys, so an older file opens and those keys still play. Names in the file are lowercase. The plain K and L keys can still play clips, because lock and exit require Ctrl and Shift. Thumbnails are shell-only previews in Application Support. Assigning a key still goes through `Show`.

## Show file

Schema version 2. JSON, at `<media>/.dee-nice/show.json`. The file holds the key map, mute, and start locked. It does not hold the media path or the display: the folder that contains `.dee-nice` is the media folder, and the shell remembers the display per folder. Unknown fields are rejected. Schema 3 or newer is rejected before the rest of the file is interpreted. Schema 1, the older Application Support file, still loads so it can be copied into the folder. The writer saves to a temporary file in the same directory and replaces the destination only after the temp file is flushed. A failed replace leaves the previous file.

The shell must refuse display mode when no output display is chosen, when an assigned file is missing, or when mpv cannot start. Those checks do not belong in the file loader: a show still has to open so the operator can fix it.

`machine.json` also stores `presentation`. A missing value means kiosk. The show file does not store it, so copying a video folder onto a kiosk does not turn presentation on.

## Where the next change goes

| Change | Start here |
| --- | --- |
| A new show edit or rule | `Show` in `crates/dee-nice-core/src/show.rs`, with a unit test |
| A new durable field | `FileShow` in `persist.rs`. Destructuring is exhaustive on purpose. Add a round trip and a rejection test. |
| The setup window | `apps/desktop/src/app.rs`. Call the core methods. Do not keep a second copy of the key map. |
| Display mode | `apps/desktop/src/session.rs` and `playback.rs`. Translate key names there: core `space` is mpv `SPACE`, core `f1` is mpv `F1`, and Ctrl+Shift+K is `ctrl+K`. |
| A lighter derivative | `apps/desktop/src/derivatives.rs`, following `docs/derivatives.md`. ffmpeg writes into the Application Support cache. The core keeps storing a relative clip path. |

Monitor enumeration and the folder scan stay in the shell. The core stores the operator's choice and the relative paths.

The shipping app is a macOS bundle, built by `scripts/bundle-macos.sh` into `dist/dee-nice.app`. Finder shows one icon. Inside `Contents/MacOS`, next to `dee-nice`, are `ffmpeg`, `ffprobe`, and a nested `mpv.app`. ffmpeg's libraries sit in `Contents/lib`, which is where that ffmpeg build already looks. mpv keeps its own libraries inside `mpv.app`. They are still separate processes. They are not linked into the Rust binary.

The shell resolves a bundled tool first, then `DEE_NICE_MPV`, `DEE_NICE_FFMPEG`, or `DEE_NICE_FFPROBE`, then `PATH`. `PATH` is a developer fallback. `cargo run` has no sibling tools, so the overrides still apply there.
