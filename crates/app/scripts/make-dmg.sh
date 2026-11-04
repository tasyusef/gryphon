#!/bin/zsh
# Packs an app into Gryphon's disk image: it opens on a vellum window with Gryphon on the left,
# Applications on the right and an arrow between (icons/installer/, drawn by installer-art.py),
# and the disk wears Gryphon's icon. package-macos.sh runs it on the stapled app, then signs and
# notarizes the result.
#
#   make-dmg.sh <Gryphon.app> <out.dmg>
#
# Finder lays the window out, so the first run asks to let Terminal control Finder: say OK.
set -euo pipefail

APP=$1 OUT=$2
ART=${0:A:h:h}/icons
VOL=/Volumes/Gryphon
# installer-art.py's DMG_W x DMG_H and its icons' centres. Finder's bounds take in the title
# bar, about 32 points. Newer Finders open the window at the user's own size instead.
W=600 H=$((380 + 32)) APP_X=150 APPLICATIONS_X=450 ICON_Y=200

[[ -d $VOL ]] && { echo "A disk called Gryphon is open. Eject it, then run this again." >&2; exit 1; }

STAGE=$(mktemp -d)
RW="$STAGE.dmg"
cp -R "$APP" "$STAGE/Gryphon.app"
ln -s /Applications "$STAGE/Applications"
mkdir "$STAGE/.background"
cp "$ART/installer/dmg-background.tiff" "$STAGE/.background/background.tiff"
hdiutil create -volname Gryphon -srcfolder "$STAGE" -fs HFS+ -format UDRW -ov "$RW" -quiet
rm -rf "$STAGE"

hdiutil attach "$RW" -readwrite -noverify -noautoopen -mountpoint "$VOL" -quiet
osascript <<EOF
tell application "Finder"
  tell disk "Gryphon"
    open
    set current view of container window to icon view
    set toolbar visible of container window to false
    set statusbar visible of container window to false
    set the bounds of container window to {200, 120, 200 + $W, 120 + $H}
    set options to the icon view options of container window
    set arrangement of options to not arranged
    set icon size of options to 128
    set text size of options to 13
    set background picture of options to file ".background:background.tiff"
    set position of item "Gryphon.app" of container window to {$APP_X, $ICON_Y}
    set position of item "Applications" of container window to {$APPLICATIONS_X, $ICON_Y}
    close
    -- Finder only keeps a window's size once it has been reopened and resized, so nudge it
    open
    delay 1
    set the bounds of container window to {200, 120, 190 + $W, 110 + $H}
  end tell
  delay 1
  tell disk "Gryphon"
    set the bounds of container window to {200, 120, 200 + $W, 120 + $H}
    update without registering applications
  end tell
  delay 2
  tell disk "Gryphon" to close
end tell
EOF
# Finder writes the layout to .DS_Store as the window closes
for _ in {1..20}; do [[ -f $VOL/.DS_Store ]] && break; sleep 0.5; done
[[ -f $VOL/.DS_Store ]] || { hdiutil detach "$VOL" -quiet; echo "Finder didn't save the window's layout" >&2; exit 1; }
# the disk's icon goes in last: Finder deletes a .VolumeIcon.icns while it lays the window out
cp "$ART/icon.icns" "$VOL/.VolumeIcon.icns"
SetFile -a C "$VOL"
rm -rf "$VOL/.fseventsd"
sync
hdiutil detach "$VOL" -quiet
hdiutil convert "$RW" -format UDZO -imagekey zlib-level=9 -ov -o "$OUT" -quiet
rm -f "$RW"
echo "Made $OUT"
