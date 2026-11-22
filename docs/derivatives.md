# Sources, derivatives, and the show

The video folder is the thing a person copies. Lighter copies are a cache on this Mac. They are never written beside the originals, and they never replace them.

The show file in the video folder, the per-Mac display memory, and the derivative cache are built. The cache lives in Application Support. The video folder is never written except for `.dee-nice/show.json`.

## Three places

| What | Where | Travels with the videos |
| --- | --- | --- |
| Source clips | The folder the operator chose | Yes |
| The show | `<folder>/.dee-nice/show.json` | Yes, because Finder copies the folder, including the hidden directory |
| Derivatives, the last folder, and this Mac's display | `~/Library/Application Support/dee-nice/` | No |

The show holds the key map, mute, and start locked. It does not hold an absolute media path. The folder that contains `.dee-nice` is the media folder. It does not hold the display. A display name from another Mac would block playback on this one.

`~/Library/Application Support/dee-nice/machine.json` remembers the last folder, the display chosen for that folder, and whether this Mac uses presentation mode. One connected display is still chosen automatically. A missing saved display asks for another instead of refusing to play.

The derivative cache is `~/Library/Application Support/dee-nice/Derivatives/`. One current file per source. The name is a hash of the canonical source path, the file size, and the modification time. A replaced source misses the cache and is made again. Entries whose source file is gone are deleted. Entries for clips that are no longer assigned can be deleted. Clips that were never assigned are never transcoded.

The scanner already skips dotfiles, so `.dee-nice` and `show.json` do not appear as videos.

## Moving machines

Copy the video folder. That is the whole show. Leave the Application Support folder behind.

On the new Mac, dee-nice opens that folder, chooses a connected display, and makes new derivatives for that Mac. A powerful Mac's copies are not copied onto a weaker one, and a weaker Mac's copies are not what the powerful one plays. Going back the other way plays the sources when they already fit, or makes a sharper local copy.

The existing Application Support show is read once when this lands, and written into the folder it points at. After that, the folder file is the show.

## When the source plays

The app decides per file, automatically, from this Mac and the chosen display. It does not play the file once to see if it stutters.

The fit is the smaller of the display's pixel long edge and this Mac's cap. The cap is 1920 when Low Power Mode is on, the Mac is Intel, or it has 8 GB of memory or less. Otherwise, on Apple Silicon, the cap is 3840.

The source plays, and nothing is written, when both of these are true:

- its codec is H.264 or HEVC
- its long edge is less than or equal to that fit

A 1080p H.264 file on a Mac whose fit is 1080p or 4K plays as itself. A 4K file on a 1080p projector, or on an 8 GB Mac, does not. A ProRes file at 1080p does not, because the codec is the expensive part. A source is never upscaled to fill a larger display.

## What assigning a key changes

Assigning a key does not change that decision. It changes whether the app does the work.

The first key assigned to a file that does not fit starts one lighter copy in the background. A second key for the same file does not start a second copy. Clearing the last key for that file cancels an unfinished copy and deletes a finished one. Files that are not on a key are never transcoded.

ffmpeg writes H.264 with `h264_videotoolbox`, AAC stereo, and `yuv420p`, into the cache, at the fit size. The source file is only read. If VideoToolbox cannot encode, the failure is visible and playback uses the source.

Entering display mode does not wait for those copies. A key whose copy is not ready yet plays the source. The next press of that key plays the copy once it exists. The file that is already on screen is not cut over to the copy mid-play.

Changing the display, or Low Power Mode, recalculates the fit. Copies for the old fit are dropped, and assigned files that no longer fit start again.

The setup window says what this Mac will do, in one line, such as "This Mac plays lighter copies, up to 1080p." A pinned size can come later. The guess is the product for now.

## What stays out of the core

Hardware, display pixels, ffmpeg, and the cache path are shell concerns. The core keeps storing a relative clip path. Resolving that path to a derivative happens in the shell at playback time.
