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

Choose a folder, then Rescan if the folder changes on disk. Videos appear as thumbnails. Drag one onto a key. The key then shows that thumbnail and its letter or digit. Click a thumbnail to select it, then click a key to assign that selection. Click the same key again, or right-click it, to clear it. The same clip may sit on more than one key.

The keys are laid out like a keyboard: digits on the top row, then Q through P, A through L, and Z through M. Function keys, space, and the arrows are not on the grid. A show file that already uses those keys still opens, and those keys still play.

The display menu lists connected screens, primary first, using pixel size. Mute, Start locked, and Presentation are checkboxes. Presentation is off by default. Kiosk hides this window while the show plays. With a second display, Presentation leaves the window there, marks the live key, and offers Lock keys and Leave display mode. With one display, Presentation still hides the window. A line under the checkboxes says whether this Mac prepares lighter copies up to 1080p or up to 4K. While a copy is being made, the window says which file is being prepared. Enter display mode does not wait for it. Start locked plays the first assigned clip when display mode opens and ignores clip keys until Ctrl+Shift+L. Without it, display mode starts on black until a key.

Enter display mode stays disabled until a folder, a connected display, and at least one resolvable clip are set. The reason is shown beside the button. Each folder has its own show. A missing assignment is reported there rather than deleted. A saved display that is not connected is cleared. One connected display is selected on its own.

The footer shows the show-file path inside the folder, and "mpv was not found." when the app cannot find a bundled mpv, `DEE_NICE_MPV`, or `mpv` on `PATH`.
