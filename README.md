# Gryphon

Gryphon installs and updates addons for World of Warcraft: Forever. It pulls
them straight from each addon's GitHub releases, so there's no CurseForge, no
account and no ads. macOS, Windows and Linux.

This repository holds the release builds. What the app does and how to use it
are at **[timothyali.com/gryphon](https://www.timothyali.com/gryphon/)**.

## Download

Grab the latest build from the [Releases](../../releases/latest) page.

| Platform | File |
| --- | --- |
| macOS, Apple silicon and Intel | `Gryphon_<version>_mac-universal.dmg` |
| Windows, x64 | `Gryphon_<version>_x64-setup.exe` |
| Linux, x86_64 | `Gryphon_<version>_amd64.AppImage` or `Gryphon_<version>_amd64.deb` |

Every release ships a `SHA256SUMS` file. Run `shasum -a 256 <file>` and compare
the result with that file's entry in `SHA256SUMS`.

### macOS

Builds are signed with a Developer ID and notarized by Apple. Open the dmg
and drag Gryphon to Applications.

### Windows

The installer isn't signed yet, so the first time you run it Windows shows a
blue "Windows protected your PC" box. Click **More info**, then **Run anyway**.

### Linux

The AppImage needs to be executable once: `chmod +x Gryphon_*.AppImage`.
The deb installs with `sudo dpkg -i Gryphon_*.deb`.

## Tome

Tome, the addon that puts one row of tabs across your Forever windows, lives
in its own repository at [tasyusef/tome](https://github.com/tasyusef/tome).
