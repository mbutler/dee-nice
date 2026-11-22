# dee-nice

dee-nice is an updated Rave Kontrol. An operator uses it to play a folder of videos from the keyboard: kiosks, galleries, art installations, and a simple show-control desk.

The person at the keyboard is the operator. Visitors are not. Display mode has to keep playing if someone mashes keys, and the operator has to be able to get back to setup.

## First milestone

One show, played on one chosen display:

1. Point the app at a directory of video files.
2. Assign one clip to a key by dragging its thumbnail onto the keyboard. The same clip may sit on more than one key. The keyboard is letters and digits.
3. Choose which monitor goes fullscreen, and whether the show is muted.
4. Enter display mode. A key plays that clip. The clip loops until another assigned key cuts to a different clip.
5. Ctrl+Shift+L locks and unlocks clip switching. While locked, the current clip keeps looping and clip keys do nothing.
6. Ctrl+Shift+K leaves display mode and returns to setup. It works while switching is locked.

The show file lives in the video folder at `.dee-nice/show.json`. It remembers the layout, the key map, which slot each key plays into, the short name on each key, which keys cut to black, mute, whether display mode should start locked, and whether clip keys lock again after a quiet interval. Copying the folder copies the show. The live lock can change during a show without rewriting that setting. Auto-lock is off until the show turns it on. The setup window then uses 60 seconds.

This Mac remembers the last folder and the display for that folder in `~/Library/Application Support/dee-nice/machine.json`. A display that is not connected is dropped, and one connected display is chosen automatically. The same file remembers whether this Mac is in kiosk or presentation. Kiosk is the default: display mode hides the setup window. Presentation keeps that window on another display when one is connected, and hides it when this Mac has only one screen. An older show file at `show.dee-nice.json` in that same Application Support folder is copied into the video folder once and then left in place.

The layout defaults to one full picture. Side by side, stacked, four squares, and two tall portrait frames divide that same display. A key still plays one video, into one slot. Pressing it hard-cuts that slot. The other slots keep looping. An empty slot stays black.

Display mode keeps this Mac awake. The screen stays on, and the Mac does not idle-sleep, until the show ends.

Start locked copies into the session when display mode opens. The session then plays the first assignment in each slot, in key order, and ignores clip keys until Ctrl+Shift+L. That assignment can be a clip or black. An unlocked start waits on black. A key assigned to black cuts its slot to black and stays in the show. A name stored on a key is what the operator reads on that key.

Failure that must stay true: a failed save leaves the previous show file untouched, and a failed enter into display mode leaves the operator in setup with the last saved show intact.

An assigned clip that already fits this Mac and this display plays as itself. A clip that does not fit gets one lighter H.264 copy, made in the background and kept outside the video folder. Enter display mode does not wait for that copy.

## Deferred

- A pinned derivative size. The automatic fit in `docs/derivatives.md` is the product for now.
- Crossfades, layers, and a mixing bus. The first version plays one clip at a time and hard-cuts.
- MIDI, OSC, and other external triggers.
- Windows and Linux as verified targets. The core stays free of macOS-only code so those can follow.

## What a person downloads

A person who is not a developer gets one Mac application. They open it. They do not install mpv, ffmpeg, Homebrew, or anything else.

That application is a `dee-nice.app` bundle. Finder shows it as one icon. Inside it, next to the dee-nice binary, are private copies of mpv and ffmpeg. The app looks beside itself before it looks on `PATH`. `scripts/bundle-macos.sh` builds that bundle for a local proof. A build of mpv with the windowing backend disabled cannot be the copy we ship. dee-nice is licensed under the GNU General Public License, version 3 or any later version. Notarization is still required before anyone else opens a downloaded copy.

A single Mach-O file with those tools packed inside and unpacked on launch is possible, and it is the wrong shape on macOS. Gatekeeper, notarization, and library paths expect an app bundle. The download is one `.dmg`: `scripts/bundle-macos.sh` writes `dist/dee-nice.dmg`, with the app and an Applications shortcut inside.

While developing, `DEE_NICE_MPV` may point at a local mpv, and `DEE_NICE_FFMPEG` / `DEE_NICE_FFPROBE` may point at local tools. Those overrides are not the product.

## Platforms

macOS is the system we run and check. The show file is JSON, schema version 4, at `.dee-nice/show.json` inside the video folder. Schemas 3, 2, and 1 still open. A schema 3 file opens as a full-screen layout. Schema 1 is the older Application Support file and can be copied in.
