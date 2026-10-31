#!/bin/zsh
# Builds the Linux x64 AppImage and deb on this Mac, in Docker with Toolbox's build image
# (toolbox-linux-build:x64: Ubuntu 22.04, Tauri's libraries, the same tauri-cli and trunk).
# x64 runs emulated on Apple silicon, so it's slow the first time; the Docker volume
# gryphon-linux-x64 keeps Cargo's build folder for next time.
#
# The repo is mounted read-only and copied into the volume, so nothing here is written to
# except target/dist.
# Output: target/dist/Gryphon_<version>_amd64.AppImage and .deb.
set -euo pipefail

ROOT=${0:A:h:h:h:h}
mkdir -p "$ROOT/target/dist"
# The page is WebAssembly, the same on every system, so it's built here and reused in the
# container (whose trunk would otherwise download wasm-opt, which kept timing out).
(cd "$ROOT/crates/ui" && trunk build --release)
docker run --rm --platform linux/amd64 -v "$ROOT":/src:ro -v gryphon-linux-x64:/work -v "$ROOT/target/dist":/dist \
  -e XDG_CACHE_HOME=/work/cache toolbox-linux-build:x64 sh -c '
set -eu
rsync -a --delete --exclude /target --exclude /crates/app/gen --exclude /.git /src/ /work/src/
# Emulated x64 refuses to run an AppImage whose header carries the "AI 2" magic bytes
# ("Exec format error"). Tauri zeroes them on linuxdeploy but not on its AppImage plugin,
# so fetch the plugin into Tauri'"'"'s cache first and zero them here.
plugin=/work/cache/tauri/linuxdeploy-plugin-appimage.AppImage
mkdir -p /work/cache/tauri
[ -f "$plugin" ] || curl -sSfL -o "$plugin" \
  https://github.com/linuxdeploy/linuxdeploy-plugin-appimage/releases/download/continuous/linuxdeploy-plugin-appimage-x86_64.AppImage
printf "\000\000\000" | dd of="$plugin" bs=1 seek=8 count=3 conv=notrunc status=none
chmod +x "$plugin"
cd /work/src/crates/app
CARGO_TARGET_DIR=/work/target cargo tauri build --bundles deb,appimage --config "{\"build\":{\"beforeBuildCommand\":null}}"
cp /work/target/release/bundle/deb/*.deb /work/target/release/bundle/appimage/*.AppImage /dist/'
echo "Done: target/dist"
