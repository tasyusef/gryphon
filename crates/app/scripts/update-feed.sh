#!/bin/sh
# The feed Gryphon's own update check reads (`src/update.rs`): signs every package the updater
# installs with the update key, then writes `latest.json` for the GitHub release.
#
#   darwin-aarch64          Gryphon_<version>_mac-universal.app.tar.gz   (package-macos.sh)
#   darwin-x86_64           the same file: the Mac app is universal
#   windows-x86_64          Gryphon_<version>_x64-setup.exe              (package-windows.sh)
#   linux-x86_64-appimage   Gryphon_<version>_amd64.AppImage             (package-linux.sh)
#   linux-x86_64-deb        Gryphon_<version>_amd64.deb
#
# Run it yourself once all four files are in target/dist. It asks for the key's password once
# (or reads TAURI_SIGNING_PRIVATE_KEY_PASSWORD). The key is $GRYPHON_UPDATER_KEY, by default
# Toolbox's ~/.tauri/toolbox-updater.key, whose public half is `plugins.updater.pubkey` in
# tauri.conf.json. Each signature names the version, which the app requires
# (`requireSignedVersion`), so an old package can't be passed off as a new one.
#
# Output, in target/dist: a `.sig` beside each package, and `latest.json`. Upload `latest.json`
# and the `.app.tar.gz` with the release's other files on tasyusef/gryphon; the app finds the
# feed at releases/latest/download/latest.json.
set -eu

app_dir=$(cd "$(dirname "$0")/.." && pwd)
root=$(cd "$app_dir/../.." && pwd)
dist="$root/target/dist"
key="${GRYPHON_UPDATER_KEY:-$HOME/.tauri/toolbox-updater.key}"
version=$(sed -n 's/^version = "\(.*\)"$/\1/p' "$root/Cargo.toml" | head -n 1)
base="https://github.com/tasyusef/gryphon/releases/download/v$version"
mac="Gryphon_${version}_mac-universal.app.tar.gz"

packages="darwin-aarch64 $mac
darwin-x86_64 $mac
windows-x86_64 Gryphon_${version}_x64-setup.exe
linux-x86_64-appimage Gryphon_${version}_amd64.AppImage
linux-x86_64-deb Gryphon_${version}_amd64.deb"

[ -f "$key" ] || { echo "No update key at $key" >&2; exit 1; }
missing=0
echo "$packages" | while read -r target file; do
  [ -f "$dist/$file" ] || { echo "Missing $dist/$file ($target)" >&2; exit 1; }
done || missing=1
[ "$missing" = 0 ] || exit 1

if [ -z "${TAURI_SIGNING_PRIVATE_KEY_PASSWORD+set}" ]; then
  printf 'Password for %s: ' "$key" >&2
  stty -echo 2>/dev/null || true
  read -r TAURI_SIGNING_PRIVATE_KEY_PASSWORD
  stty echo 2>/dev/null || true
  echo >&2
fi
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD

platforms=""
for line in $(echo "$packages" | tr ' ' '='); do
  target=${line%%=*}
  file=${line#*=}
  # Both Mac targets point at the one universal package: sign it once.
  if [ "$target" != darwin-x86_64 ]; then
    rm -f "$dist/$file.sig"
    cargo tauri signer sign -f "$key" --app-version "$version" "$dist/$file" >/dev/null
    echo "Signed $file"
  fi
  signature=$(cat "$dist/$file.sig")
  entry=$(printf '    "%s": {\n      "signature": "%s",\n      "url": "%s/%s"\n    }' "$target" "$signature" "$base" "$file")
  platforms="${platforms:+$platforms,
}$entry"
done

cat > "$dist/latest.json" <<EOF
{
  "version": "$version",
  "notes": "",
  "pub_date": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "platforms": {
$platforms
  }
}
EOF
echo "Wrote $dist/latest.json"
