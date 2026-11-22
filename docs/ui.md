# Setup window

The setup window is the only dee-nice window. Display mode hides it and mpv covers the chosen display. Ctrl+Shift+K shows the window again.

The window opens at 1360×860 and will not shrink below 1180×720. The theme is dark.

| Token | RGB | Use |
| --- | --- | --- |
| Background | 22, 23, 26 | Window fill |
| Panel | 30, 32, 36 | Panels |
| Primary | 232, 230, 223 | Enter display mode |
| Primary text | 24, 24, 24 | Label on that button |
| Assigned | 46, 58, 72 | A key that has a clip |
| Drop | 78, 92, 110 | A key under a dragged video |
| Error fill | 72, 32, 32 | The notice banner |
| Error text | 255, 214, 214 | Notice text |

## Interaction

Choose a folder, then Rescan if the folder changes on disk. Videos appear as thumbnails, with a Black card first. Drag a thumbnail or Black onto a key. The key then shows that thumbnail, or a black fill, and its letter or digit. Click a thumbnail or Black to select it, then click a key to assign that selection. Click an assigned key to select it for a name. The Name field writes up to 24 characters onto that key, and the name is drawn on the key. Right-click a key to clear it. The same clip may sit on more than one key.

The layout row sits above the keyboard: Full, Side by side, Stacked, Four, and Portraits. Full is the default and hides the slot row. The other layouts show a slot row. Select a slot, then assign keys to it. A divided key shows a small slot mark. Changing the layout while a show is playing is disabled. The keys are laid out like a keyboard: digits on the top row, then Q through P, A through L, and Z through M. Function keys, space, and the arrows are not on the grid. A show file that already uses those keys still opens, and those keys still play.

The display menu lists connected screens, primary first, using pixel size. Mute, Start locked, Auto-lock, and Presentation are checkboxes. Auto-lock is off until the show turns it on, and then clip keys lock again after 60 seconds with no key. Presentation is off by default. Kiosk hides this window while the show plays. With a second display, Presentation leaves the window there, marks the live key, and offers Lock keys and Leave display mode. With one display, Presentation still hides the window. A line under the checkboxes says whether this Mac prepares lighter copies up to 1080p or up to 4K. While a copy is being made, the window says which file is being prepared. Enter display mode keeps this Mac awake until the show ends, and it does not wait for a lighter copy. Start locked plays the first assignment in each slot when display mode opens and ignores clip keys until Ctrl+Shift+L. That assignment can be black. Without Start locked, display mode starts on black until a key. In a divided layout, a key hard-cuts only its slot.

Enter display mode stays disabled until a folder, a connected display, and at least one resolvable clip are set. The reason is shown beside the button. Each folder has its own show. A missing assignment is reported there rather than deleted. A saved display that is not connected is cleared. One connected display is selected on its own.

The footer shows the show-file path inside the folder, and "mpv was not found." when the app cannot find a bundled mpv, `DEE_NICE_MPV`, or `mpv` on `PATH`.
