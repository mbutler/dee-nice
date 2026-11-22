#!/bin/bash
# Assemble dist/dee-nice.app. Finder shows one icon. mpv and ffmpeg are inside.
#
# Expects two local tools that are not committed:
#   .tools/mpv-macos/mpv.app          official mpv 0.42.0 macOS build
#   .tools/mpv/bin/ffmpeg             conda-forge ffmpeg with VideoToolbox
#   .tools/mpv/bin/ffprobe            and its matching ffprobe
# The conda mpv binary is not copied. It cannot open a window.
#
# This bundle is a local proof. It is not notarized, and no license has been chosen.

set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
mpv_app="$root/.tools/mpv-macos/mpv.app"
ffmpeg_bin="$root/.tools/mpv/bin/ffmpeg"
ffprobe_bin="$root/.tools/mpv/bin/ffprobe"
lib_root="$root/.tools/mpv/lib"
out="$root/dist/dee-nice.app"
macos="$out/Contents/MacOS"
lib_dir="$out/Contents/lib"

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
mkdir -p "$macos" "$lib_dir"
cp "$root/target/release/dee-nice" "$macos/dee-nice"
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

echo "Built $out"
