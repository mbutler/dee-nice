# Task 001 Show document

Status: done

## Outcome

The app can represent one show, reject an assignment that would escape the media folder, and save and reopen that show. A failed save leaves the previous file in place.

## Scope

The core document, its bindings, and the version-1 file. The setup window, mpv, and derivatives are out of scope.

## Acceptance evidence

- Assign, replace, and clear clips. The same clip may sit on two keys.
- Absolute paths, empty paths, and `..` fail and leave the show unchanged.
- Unlocked bindings play every assignment and include Ctrl+Shift+L and Ctrl+Shift+K.
- Locked bindings omit clip keys and keep both chords. `start_locked` does not itself remove play bindings.
- Save, replace, and load round-trip. Schema 2 is rejected even when the file has an unknown field. Malformed files, duplicate keys, and escaping clips fail the load.
- A save whose destination cannot be replaced leaves the previous file and no temp file.

## Relevant context

`docs/architecture.md` and `crates/dee-nice-core`.
