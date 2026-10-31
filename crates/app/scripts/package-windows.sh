#!/bin/zsh
# Builds the Windows x64 installer on this Mac: cargo-xwin cross-compiles the app, then the
# NSIS script Tauri writes is packed in Docker. Homebrew's makensis crashes on it
# (std::bad_alloc), so Tauri's own NSIS step is expected to fail here.
#
# Needs: the x86_64-pc-windows-msvc target, cargo-xwin, Homebrew's llvm, and Docker running
# with the toolbox-nsis image (Ubuntu 24.04 + nsis, from the Toolbox repo).
# Output: target/dist/Gryphon_<version>_x64-setup.exe (unsigned).
set -uo pipefail

ROOT=${0:A:h:h:h:h}
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)
NSIS="$ROOT/target/x86_64-pc-windows-msvc/release/nsis/x64"
CACHE="$ROOT/target/tauri-cache"

rm -f "$NSIS/installer.nsi"
(cd "$ROOT/crates/app" && PATH="/opt/homebrew/opt/llvm/bin:$PATH" \
  cargo tauri build --runner cargo-xwin --target x86_64-pc-windows-msvc --bundles nsis)
[[ -f "$NSIS/installer.nsi" ]] || { echo "Tauri didn't write the NSIS script" >&2; exit 1; }
set -e

# Docker can't see ~/Library, so the script points at a copy of Tauri's cache instead.
rm -rf "$CACHE" && cp -R ~/Library/Caches/tauri "$CACHE"
sed "s#$HOME/Library/Caches/tauri#$CACHE#g" "$NSIS/installer.nsi" > "$NSIS/installer-docker.nsi"
cp "$CACHE/NSIS/Plugins/x86-unicode/additional/nsis_tauri_utils.dll" "$NSIS/"
rm -f "$NSIS/nsis-output.exe"
docker run --rm -v "$ROOT:$ROOT" -w "$NSIS" toolbox-nsis:latest sh -c \
  'cp nsis_tauri_utils.dll /usr/share/nsis/Plugins/x86-unicode/ && makensis -INPUTCHARSET UTF8 -V2 installer-docker.nsi'

mkdir -p "$ROOT/target/dist"
cp "$NSIS/nsis-output.exe" "$ROOT/target/dist/Gryphon_${VERSION}_x64-setup.exe"
echo "Done: target/dist/Gryphon_${VERSION}_x64-setup.exe"
