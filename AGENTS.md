# Agent instructions

Read these instructions, `docs/status.md`, and the active task before editing.
For initial setup or a major architecture change, also read
`Rust_App_Development_Guide.md`. For routine work, read the relevant portion
of `docs/architecture.md`.

## Product and boundaries

Use `docs/product.md` for the current milestone and deferred scope.
`dee-nice-core` owns the show, validation, playback bindings, and the show
file. It has no GUI dependency and it does not start mpv. `apps/desktop` owns
the setup window and the mpv supervisor. Keep show edits on the core API.
Document consequential changes to these decisions with their rationale.

The milestone is playback: assign keys, choose a display, play fullscreen,
hard-cut, loop, lock with Ctrl+Shift+L, and exit with Ctrl+Shift+K. Derivatives
are deferred. Do not widen the milestone to mixing, MIDI, or transcoding.

## Work protocol

Inspect git status and preserve unrelated work. Complete one bounded task
with explicit acceptance evidence. Keep changes focused and use the core API
before adding a parallel key map or a second show file writer. Make routine
reversible decisions independently; surface changes to the product scope or a
consequential established contract.

Return useful errors for bad input and I/O. A failed assignment, load, or save
preserves the last valid show. Revisit `persist.rs` whenever a durable field
changes. Do not drop unknown show-file data to make a load succeed.

## Verification

During iteration, run checks for the affected package. Before completion, run:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

mpv is not part of this baseline. When display mode exists, a visual check on
macOS is required in addition to these commands. Report a missing mpv or a
missing display as not run.

## Handoff

Update `docs/status.md` with what works, the verification evidence, the
remaining limitations, and the next action. Update affected docs in the same
change. Do not weaken tests or lints to finish a task.
