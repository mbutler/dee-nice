# Task 002 Display mode

Status: done

## Outcome

The operator can assign clips in the setup window and enter display mode. mpv plays the clip fullscreen on the chosen display, hard-cuts on the next key, loops, locks, and returns to setup on Ctrl+Shift+K.

## Scope

The setup window, display list, readiness check, and the mpv supervisor. Derivatives and settings for the fixed choices are out of scope.

## Acceptance evidence

- The window opens at the setup size. Enter stays disabled until the show is ready, and the reason is shown.
- Display mode uses the official mpv 0.42.0 macOS build, fullscreen, with `--screen-name` and `--fs-screen-name` set to the display's localized name and no numeric screen index beside them.
- A then B cut from `red.mp4` to `blue.mp4`. Ctrl+Shift+L left the current clip playing. Ctrl+Shift+K exited with "Left display mode."
- That run is `session::tests::display_mode_switches_locks_and_quits`, ignored unless `DEE_NICE_MPV` is set. It passed on 22 November 2026.
- A conda-forge mpv built with Cocoa disabled cannot open the window. The session keeps mpv's stdout and stderr so that failure is visible in setup.

## Relevant context

`docs/ui.md`, `docs/architecture.md`, and `apps/desktop`.
