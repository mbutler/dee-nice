#!/bin/bash
# Copyright (C) 2026 Matthew Butler
#
# dee-nice is free software: you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation, either version 3 of the License, or
# (at your option) any later version.
#
# Assemble dist/dee-nice.app. Finder shows one icon. mpv and ffmpeg are inside.
#
# Expects two local tools that are not committed:
#   .tools/mpv-macos/mpv.app          official mpv 0.42.0 macOS build
#   .tools/mpv/bin/ffmpeg             conda-forge ffmpeg with VideoToolbox
#   .tools/mpv/bin/ffprobe            and its matching ffprobe
# The conda mpv binary is not copied. It cannot open a window.
#
# This bundle is a local proof. dee-nice is GPL-3.0-or-later. It is not notarized.
# The script also writes dist/dee-nice.dmg, the single file a person downloads.

set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
mpv_app="$root/.tools/mpv-macos/mpv.app"
ffmpeg_bin="$root/.tools/mpv/bin/ffmpeg"
ffprobe_bin="$root/.tools/mpv/bin/ffprobe"
lib_root="$root/.tools/mpv/lib"
out="$root/dist/dee-nice.app"
macos="$out/Contents/MacOS"
lib_dir="$out/Contents/lib"
resources="$out/Contents/Resources"

if [[ ! -x "$mpv_app/Contents/MacOS/mpv" ]]; then
  echo "Need the official mpv.app at .tools/mpv-macos/mpv.app" >&2
  exit 1
fi
if [[ ! -x "$ffmpeg_bin" || ! -x "$ffprobe_bin" ]]; then
  echo "Need conda-forge ffmpeg and ffprobe at .tools/mpv/bin/" >&2
  exit 1
fi

cargo build --release --locked -p dee-nice

rm -rf "$out"
mkdir -p "$macos" "$lib_dir" "$resources"
cp "$root/target/release/dee-nice" "$macos/dee-nice"
cp "$root/LICENSE" "$resources/LICENSE"
cp "$root/THIRD-PARTY-NOTICES" "$resources/THIRD-PARTY-NOTICES"
cp "$root/apps/desktop/assets/AppIcon.icns" "$resources/AppIcon.icns"
chmod +x "$macos/dee-nice"
ditto "$mpv_app" "$macos/mpv.app"
cp "$ffmpeg_bin" "$macos/ffmpeg"
cp "$ffprobe_bin" "$macos/ffprobe"
chmod +x "$macos/ffmpeg" "$macos/ffprobe"

python3 - "$ffmpeg_bin" "$ffprobe_bin" "$lib_root" "$lib_dir" <<'PY'
import os
import shutil
import subprocess
import sys

bins = sys.argv[1:3]
lib_root = sys.argv[3]
dest = sys.argv[4]
seen = set()
queue = list(bins)
while queue:
    path = queue.pop()
    if path in seen or not os.path.isfile(path):
        continue
    seen.add(path)
    output = subprocess.check_output(["otool", "-L", path], text=True)
    for line in output.splitlines()[1:]:
        dep = line.strip().split(" (", 1)[0]
        if not dep.startswith("@rpath/"):
            continue
        name = dep.split("/", 1)[1]
        candidate = os.path.join(lib_root, name)
        if os.path.isfile(candidate):
            queue.append(candidate)
        else:
            raise SystemExit(f"missing library {name} for {path}")
for path in seen:
    if path in bins:
        continue
    shutil.copy2(path, os.path.join(dest, os.path.basename(path)))
PY

cat > "$out/Contents/Info.plist" <<'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleDisplayName</key>
  <string>dee-nice</string>
  <key>CFBundleExecutable</key>
  <string>dee-nice</string>
  <key>CFBundleIconFile</key>
  <string>AppIcon</string>
  <key>CFBundleIdentifier</key>
  <string>app.dee-nice</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>dee-nice</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>0.1.0</string>
  <key>CFBundleVersion</key>
  <string>0.1.0</string>
  <key>LSApplicationCategoryType</key>
  <string>public.app-category.video</string>
  <key>LSMinimumSystemVersion</key>
  <string>12.0</string>
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
EOF

xattr -cr "$out" || true
codesign --force --deep --sign - "$out"

# The Applications shortcut has to be created on the mounted image.
# Giving hdiutil a folder that already contains that symlink makes it
# follow the link and copy the Mac's Applications folder into the image.
dmg="$root/dist/dee-nice.dmg"
work="$(mktemp -d)"
sparse="$work/dee-nice.sparseimage"
mount="$work/mnt"
mkdir -p "$mount"
cleanup() {
  if mount | grep -F -q "on $mount "; then
    hdiutil detach "$mount" >/dev/null || true
  fi
  rm -rf "$work"
}
trap cleanup EXIT
hdiutil create -size 2g -fs HFS+ -volname "dee-nice" -type SPARSE -ov "$sparse" >/dev/null
hdiutil attach -readwrite -noverify -noautoopen -mountpoint "$mount" "$sparse" >/dev/null
ditto "$out" "$mount/dee-nice.app"
ln -s /Applications "$mount/Applications"
sync
hdiutil detach "$mount" >/dev/null
rm -f "$dmg"
hdiutil convert "$sparse" -format UDZO -o "$dmg" >/dev/null

echo "Built $out"
echo "Built $dmg"
