#!/bin/bash
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
if [[ $# -lt 1 ]]; then
  echo "Usage: scripts/package_macos.sh OUTPUT_DIRECTORY [RELEASE_BINARY]" >&2
  exit 2
fi
output_dir="$1"
binary="target/aarch64-apple-darwin/release/nekosportsworldtool"
if [[ $# -ge 2 ]]; then binary="$2"; fi
if [[ "$binary" != /* ]]; then binary="$repo/$binary"; fi
if [[ "$output_dir" != /* ]]; then output_dir="$repo/$output_dir"; fi
mkdir -p "$output_dir"
output_dir="$(cd "$output_dir" && pwd)"
archive="$output_dir/NekoSportsWorldTool-J-claude-macOS-arm64.zip"
[[ ! -e "$archive" ]] || { echo "Output already exists: $archive" >&2; exit 1; }
[[ -f "$binary" ]] || { echo "Release binary missing: $binary" >&2; exit 1; }
[[ "$(uname -s)" == "Darwin" && "$(uname -m)" == "arm64" ]] || { echo "Run this packager on an Apple Silicon Mac." >&2; exit 1; }
file "$binary" | grep -q 'Mach-O 64-bit executable arm64' || { echo "Expected an arm64 Mach-O binary." >&2; exit 1; }
minimum_os="$(otool -l "$binary" | awk '/LC_BUILD_VERSION/{found=1; next} found && /minos/{print $2; exit}')"
[[ "$minimum_os" == "11.0" ]] || { echo "Expected macOS 11.0 deployment target, found: $minimum_os" >&2; exit 1; }

commit="$(git -C "$repo" rev-parse HEAD)"
version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$repo/Cargo.toml" | head -1)"
[[ -n "$version" ]] || { echo "Could not read package version." >&2; exit 1; }
binary_version="$("$binary" --version)"
[[ "$binary_version" == *"$version ($commit"* ]] || { echo "Binary version/commit does not match HEAD $commit: $binary_version" >&2; exit 1; }

tmp_base="$(getconf DARWIN_USER_TEMP_DIR)"
temporary="$(mktemp -d "$tmp_base/nswt-macos-package.XXXXXX")"
trap 'rm -rf "$temporary"' EXIT
package="$temporary/NekoSportsWorldTool-J-claude-macOS-arm64"
app="$package/NekoSportsWorldTool.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
install -m 755 "$binary" "$app/Contents/MacOS/nekosportsworldtool"
cp "$repo/assets/branding/sports-route.png" "$app/Contents/Resources/sports-route.png"

iconset="$temporary/sports-route.iconset"
mkdir -p "$iconset"
for size in 16 32 64 128 256 512; do
  sips -s format png -z "$size" "$size" "$repo/assets/branding/sports-route.png" --out "$iconset/icon_"$size"x"$size".png" >/dev/null
  double="$((size * 2))"
  sips -s format png -z "$double" "$double" "$repo/assets/branding/sports-route.png" --out "$iconset/icon_"$size"x"$size"@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$app/Contents/Resources/sports-route.icns"

python3 - "$app/Contents/Info.plist" "$version" <<'PY'
import plistlib, sys
path, version = sys.argv[1:]
info = {
    "CFBundleDevelopmentRegion": "zh_CN",
    "CFBundleExecutable": "nekosportsworldtool",
    "CFBundleIconFile": "sports-route",
    "CFBundleIdentifier": "org.nekosportsworldtool.jclaude",
    "CFBundleInfoDictionaryVersion": "6.0",
    "CFBundleName": "NekoSportsWorldTool",
    "CFBundleDisplayName": "NekoSportsWorldTool",
    "CFBundlePackageType": "APPL",
    "CFBundleShortVersionString": version,
    "CFBundleVersion": version,
    "LSMinimumSystemVersion": "11.0",
    "NSHighResolutionCapable": True,
}
with open(path, "wb") as file:
    plistlib.dump(info, file, sort_keys=True)
PY
printf 'APPL????' > "$app/Contents/PkgInfo"

cp "$repo/LICENSE" "$package/"
cp "$repo/README-Portable.md" "$package/使用说明-简体中文.md"
python3 "$repo/scripts/collect_licenses.py" --target aarch64-apple-darwin --offline --output "$package"

codesign --force --deep --sign - --timestamp=none "$app"
codesign --verify --deep --strict "$app"
signature_status="Ad-hoc signed only; no Developer ID signature or notarization"
# A portable app may depend on macOS system libraries, never local Homebrew paths.
otool -L "$app/Contents/MacOS/nekosportsworldtool" > "$temporary/dependencies.txt"
python3 - "$temporary/dependencies.txt" <<'PY_DEPS'
import sys
from pathlib import Path
for line in Path(sys.argv[1]).read_text().splitlines()[1:]:
    library = line.strip().split(" (", 1)[0]
    if library and not library.startswith(("/System/Library/", "/usr/lib/")):
        raise SystemExit("Non-system library must be bundled: " + library)
PY_DEPS
{
  printf 'Product: NekoSportsWorldTool\nVersion: %s\nCommit: %s\nTarget: aarch64-apple-darwin\nArchitecture: Apple Silicon arm64\nSignature status: %s\nMinimum macOS: 11.0 (bundle metadata)\nNotarization: none\n' "$version" "$commit" "$signature_status"
} > "$package/BUILD-INFO.txt"

(cd "$package" && find . -type f ! -name SHA256SUMS.txt -print0 | sort -z | xargs -0 shasum -a 256 > SHA256SUMS.txt)
ditto -c -k --norsrc --keepParent "$package" "$archive"
(cd "$output_dir" && shasum -a 256 "NekoSportsWorldTool-J-claude-macOS-arm64.zip" > SHA256SUMS.txt)

verify="$temporary/unpacked"
mkdir -p "$verify"
ditto -x -k "$archive" "$verify"
unpacked="$verify/NekoSportsWorldTool-J-claude-macOS-arm64"
(cd "$unpacked" && shasum -a 256 -c SHA256SUMS.txt)
python3 - "$unpacked" <<'PY_FILES'
from pathlib import Path
import sys
root = Path(sys.argv[1])
allowed = {"LICENSE", "使用说明-简体中文.md", "BUILD-INFO.txt", "SHA256SUMS.txt", "THIRD-PARTY-NOTICES.txt"}
for path in root.rglob("*"):
    if path.is_symlink():
        raise SystemExit("Unexpected symlink: " + str(path))
    if not path.is_file():
        continue
    relative = path.relative_to(root).as_posix()
    if relative not in allowed and not relative.startswith(("licenses/", "NekoSportsWorldTool.app/Contents/")):
        raise SystemExit("Unexpected packaged file: " + relative)
    if path.name.lower() in {"config.json", "session.json", "identity.json", ".ds_store"} or path.suffix.lower() == ".log":
        raise SystemExit("Private/runtime file in package: " + relative)
PY_FILES
[[ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$unpacked/NekoSportsWorldTool.app/Contents/Info.plist")" == "$version" ]]
codesign --verify --deep --strict "$unpacked/NekoSportsWorldTool.app"
"$unpacked/NekoSportsWorldTool.app/Contents/MacOS/nekosportsworldtool" --version
echo "Verified macOS portable package: $archive"
