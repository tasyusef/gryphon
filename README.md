<img src="crates/app/icons/128x128@2x.png" width="96" alt="Gryphon's icon: a yellow blackletter pixel G on a dark tile">

# Gryphon

Gryphon installs and updates addons for World of Warcraft: Forever. It pulls them straight from each addon's GitHub releases, so there's no CurseForge, no Overwolf, no account and no ads.

I made it for me and a few friends ahead of Forever's launch. It's small on purpose. You pick an addon, it grabs the Forever build, and it keeps it up to date after that.

![The Picks screen, listing OpenMap, Questie, DialogueUI and Spoken with their versions and install buttons](screenshots/1-picks.png)

There are a few more screenshots in the `screenshots` folder, including the light theme.

## What it does

- It comes with a short list of addons I've checked have real Forever builds: OpenMap, Questie, DialogueUI and Spoken, plus Spoken's quest voices.
- It can install other addons too. Paste the addon's GitHub repo, and as long as a release has a zip built for Forever, Gryphon installs it. Nothing in your AddOns folder changes until that's been checked.
- It checks for updates and installs them when you say so.
- Before an update or a removal touches anything, the old copy of the folder goes to `Interface/.gryphon-backup`, so you can always get it back.
- Addons you installed some other way are left alone. Gryphon just lists them and tells you whether Forever will load them.

It finds the game in the usual place on its own. If yours lives somewhere else, you can point it at the World of Warcraft folder in Settings.

## GitHub limits

Without signing in, GitHub allows 60 update checks an hour from your network. That's plenty for a handful of addons. If you ever run into it, make a GitHub token with no scopes and paste it into Settings. Heads up: it's saved in plain text in the app's settings file.

## Running it yourself

You'll need Rust, the WebAssembly target, Trunk and the Tauri CLI:

```bash
rustup target add wasm32-unknown-unknown
```

```bash
cargo install trunk tauri-cli
```

Then start the app from `crates/app`:

```bash
cd crates/app && cargo tauri dev
```

`cargo tauri build` in the same folder makes a proper app you can drag into Applications.

If you only want to look at the interface, `trunk serve --config crates/ui/Trunk.toml --port 5190` opens it in a browser with some made-up addons to click around.

## From the terminal

The same engine works without the window:

```bash
cargo run -p gryphon-cli -- help
```

`status` shows the game and what's installed, `list` shows the built-in picks, `check` looks for updates, and `install`, `update` and `remove` do what they say. `install` takes a pick's name (like `openmap`) or any GitHub repo. Set `GITHUB_TOKEN` if you want the higher limit here too.

## How it's laid out

- `crates/core` does the real work: finding the game, reading `.toc` files, talking to GitHub and installing.
- `crates/app` is the desktop window, built with Tauri.
- `crates/ui` is the page inside that window, written in Leptos and built with Trunk.
- `crates/cli` is the terminal version.
- `crates/shared` holds the types the app and the page both use.
- `RESEARCH.md` has my notes from before I started: which addons have Forever builds, and how other managers handle things.
