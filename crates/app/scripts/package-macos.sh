#!/bin/zsh
# Builds Gryphon for Mac as one universal app (Apple silicon and Intel), signs it with the
# Developer ID, notarizes it with Apple and staples the ticket, then packs a dmg, a zip and the
# updater's .app.tar.gz into target/dist/. Then run update-feed.sh to sign the updates.
#
# Run it yourself from the repo root: it uses the login Keychain and Apple's notary service.
# It reuses Toolbox's notary profile `toolbox` (same team), made once with:
#   xcrun notarytool store-credentials toolbox --apple-id <your Apple ID> --team-id HXQFMFQ3D6
# If codesign asks for the certificate's key, choose Always Allow.
set -euo pipefail

ROOT=${0:A:h:h:h:h}
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)
export APPLE_SIGNING_IDENTITY="Developer ID Application: Timothy Smith Yusef (HXQFMFQ3D6)"
APP="$ROOT/target/universal-apple-darwin/release/bundle/macos/Gryphon.app"
DIST="$ROOT/target/dist"
NAME="Gryphon_${VERSION}_mac-universal"

cd "$ROOT/crates/app"
cargo tauri build --target universal-apple-darwin --bundles app

# Apple checks a zip of the app; the ticket is then stapled to the app itself.
mkdir -p "$DIST"
ditto -c -k --norsrc --keepParent "$APP" "$DIST/$NAME.zip"
xcrun notarytool submit "$DIST/$NAME.zip" --keychain-profile toolbox --wait
xcrun stapler staple "$APP"
spctl --assess --type execute --verbose=2 "$APP"

# Pack again so the zip holds the stapled app, and make the dmg from it too.
rm "$DIST/$NAME.zip"
ditto -c -k --norsrc --keepParent "$APP" "$DIST/$NAME.zip"
# The package the app's own updater installs, for Apple silicon and Intel alike; update-feed.sh
# signs it. No resource forks or extended attributes, so the copy it unpacks matches the signature.
rm -f "$DIST/$NAME.app.tar.gz"
COPYFILE_DISABLE=1 tar -C "${APP:h}" --no-mac-metadata --no-xattrs -czf "$DIST/$NAME.app.tar.gz" Gryphon.app
"$ROOT/crates/app/scripts/make-dmg.sh" "$APP" "$DIST/$NAME.dmg"
codesign --sign "$APPLE_SIGNING_IDENTITY" --timestamp "$DIST/$NAME.dmg"
xcrun notarytool submit "$DIST/$NAME.dmg" --keychain-profile toolbox --wait
xcrun stapler staple "$DIST/$NAME.dmg"

echo "Done: $DIST/$NAME.dmg, $DIST/$NAME.zip and $DIST/$NAME.app.tar.gz"
