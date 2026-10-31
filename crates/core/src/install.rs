//! Installing and removing. A download is unpacked beside AddOns and checked for a Forever
//! build before anything in AddOns changes. Folders it replaces move to a backup folder first,
//! and come back if a step fails, so an addon is never left half replaced. Nothing is deleted
//! outright: the backup keeps the last copy of each folder.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use gryphon_shared::{Fit, Folder, Source, Step};

use crate::error::{Result, msg};
use crate::github::{Client, Latest};
use crate::state::{Installed, State};
use crate::toc;
use crate::wow::Game;

/// Downloads and unpacked copies, under Interface so a rename into AddOns never crosses disks.
const WORK_DIR: &str = ".gryphon-work";
/// The last copy of every folder an install replaced or a removal took out.
pub const BACKUP_DIR: &str = ".gryphon-backup";

pub type OnStep<'a> = &'a mut (dyn FnMut(Step) + Send);

/// Installs or updates `id` to the newest Forever release of `source`.
pub async fn install(client: &Client, game: &Game, id: &str, name: &str, source: &Source, on_step: OnStep<'_>) -> Result<Installed> {
    on_step(Step::Checking);
    let latest = client.latest(source).await?;
    install_release(client, game, id, name, source, &latest, on_step).await
}

/// Installs one release already looked up with [`Client::latest`].
pub async fn install_release(
    client: &Client,
    game: &Game,
    id: &str,
    name: &str,
    source: &Source,
    latest: &Latest,
    on_step: OnStep<'_>,
) -> Result<Installed> {
    let work = game.interface_dir().join(WORK_DIR);
    fs::create_dir_all(&work)?;
    let key: String = id.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect();
    let zip = work.join(format!("{key}.zip"));
    let unpacked = work.join(format!("{key}.unpacked"));
    let outcome = fetch_and_place(client, game, id, name, source, latest, &zip, &unpacked, on_step).await;
    let _ = fs::remove_file(&zip);
    let _ = fs::remove_dir_all(&unpacked);
    let _ = fs::remove_dir(&work); // only when empty
    outcome
}

#[allow(clippy::too_many_arguments)]
async fn fetch_and_place(
    client: &Client,
    game: &Game,
    id: &str,
    name: &str,
    source: &Source,
    latest: &Latest,
    zip: &Path,
    unpacked: &Path,
    on_step: OnStep<'_>,
) -> Result<Installed> {
    let mut reported = 0u64;
    client
        .download(&latest.asset.browser_download_url, zip, |done, total| {
            // a step every 1% (at least every MB), so a 1.5 GB pack does not flood the UI
            let every = total.map_or(1 << 20, |t| (t / 100).max(1 << 20));
            if done == 0 || done - reported >= every || Some(done) == total {
                reported = done;
                on_step(Step::Downloading { done, total });
            }
        })
        .await?;

    on_step(Step::Unpacking);
    let (zip_path, out) = (zip.to_path_buf(), unpacked.to_path_buf());
    let (base, folders) = tokio::task::spawn_blocking(move || unpack(&zip_path, &out)).await.map_err(|e| msg(e.to_string()))??;

    if !folders.iter().any(|f| f.fit == Fit::Forever) {
        let found: BTreeSet<u32> = folders.iter().flat_map(|f| f.interface.iter().copied()).collect();
        let found = if found.is_empty() { "no interface number".to_string() } else { found.iter().map(u32::to_string).collect::<Vec<_>>().join(", ") };
        return Err(msg(format!("{name} {} is not built for WoW Forever (it lists {found}). Nothing was changed.", latest.tag)));
    }
    let names: Vec<String> = folders.into_iter().filter(|f| f.fit != Fit::NoToc).map(|f| f.name).collect();

    let mut state = State::load(game)?;
    place(game, id, &names, &base, &mut state)?;
    let installed = Installed {
        name: name.to_string(),
        source: source.clone(),
        tag: latest.tag.clone(),
        asset: latest.asset.name.clone(),
        folders: names,
        installed_at: now(),
    };
    state.addons.insert(id.to_string(), installed.clone());
    state.save(game)?;
    on_step(Step::Done);
    Ok(installed)
}

/// Unpacks a zip into `dest` and reads each top-level folder. Refuses entries that would land
/// outside `dest`; skips loose top-level files and macOS metadata. A zip that wraps its
/// addons in one extra folder is unwrapped.
fn unpack(zip: &Path, dest: &Path) -> Result<(PathBuf, Vec<Folder>)> {
    let _ = fs::remove_dir_all(dest);
    fs::create_dir_all(dest)?;
    let mut archive = zip::ZipArchive::new(fs::File::open(zip)?)?;
    let mut tops = BTreeSet::new();
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let Some(path) = entry.enclosed_name() else {
            let shown = entry.name().map(|n| n.into_owned()).unwrap_or_default();
            return Err(msg(format!("The download has an unsafe file path ({shown}). Nothing was changed.")));
        };
        let mut parts = path.components();
        let Some(Component::Normal(top)) = parts.next() else { continue };
        let top = top.to_string_lossy().into_owned();
        if top == "__MACOSX" || top.starts_with('.') || (parts.next().is_none() && !entry.is_dir()) {
            continue;
        }
        tops.insert(top);
        let out = dest.join(&path);
        if entry.is_dir() {
            fs::create_dir_all(&out)?;
            continue;
        }
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent)?;
        }
        std::io::copy(&mut entry, &mut fs::File::create(&out)?)?;
    }

    let read = |base: &Path, names: &BTreeSet<String>| -> Vec<Folder> { names.iter().map(|n| toc::read_folder(&base.join(n))).collect() };
    let folders = read(dest, &tops);
    if let [only] = folders.as_slice()
        && only.fit == Fit::NoToc
    {
        let inner = dest.join(&only.name);
        let children: BTreeSet<String> = fs::read_dir(&inner)?
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        let nested = read(&inner, &children);
        if nested.iter().any(|f| f.fit != Fit::NoToc) {
            return Ok((inner, nested));
        }
    }
    Ok((dest.to_path_buf(), folders))
}

/// Swaps `names` from `base` into AddOns. Every folder it replaces, and every folder the
/// addon had before but no longer ships, moves to the backup first.
fn place(game: &Game, id: &str, names: &[String], base: &Path, state: &mut State) -> Result<()> {
    let addons = game.addons_dir();
    let backup = game.interface_dir().join(BACKUP_DIR);
    fs::create_dir_all(&addons)?;
    fs::create_dir_all(&backup)?;

    let had = state.addons.get(id).map(|a| a.folders.clone()).unwrap_or_default();
    for gone in had.iter().filter(|f| !names.iter().any(|n| n.eq_ignore_ascii_case(f))) {
        back_up(&addons, &backup, gone)?;
    }
    for name in names {
        let saved = back_up(&addons, &backup, name)?;
        if let Err(e) = fs::rename(base.join(name), addons.join(name)) {
            if saved {
                let _ = fs::rename(backup.join(name), addons.join(name));
            }
            return Err(e.into());
        }
        // a folder another addon here shipped now belongs to this one
        for (_, installed) in state.addons.iter_mut().filter(|(other, _)| other.as_str() != id) {
            installed.folders.retain(|f| !f.eq_ignore_ascii_case(name));
        }
    }
    Ok(())
}

/// Moves `AddOns/name` into the backup, over any older copy. False when there was nothing.
fn back_up(addons: &Path, backup: &Path, name: &str) -> Result<bool> {
    let from = addons.join(name);
    if !from.exists() {
        return Ok(false);
    }
    let to = backup.join(name);
    if to.exists() {
        fs::remove_dir_all(&to)?;
    }
    fs::rename(from, to)?;
    Ok(true)
}

/// Takes an addon out: its folders move to the backup and it leaves the record.
pub fn remove(game: &Game, id: &str) -> Result<Installed> {
    let mut state = State::load(game)?;
    let installed = state.addons.remove(id).ok_or_else(|| msg(format!("{id} is not installed.")))?;
    let backup = game.interface_dir().join(BACKUP_DIR);
    fs::create_dir_all(&backup)?;
    for folder in &installed.folders {
        back_up(&game.addons_dir(), &backup, folder)?;
    }
    state.save(game)?;
    Ok(installed)
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn game() -> (tempfile::TempDir, Game) {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("_forever_/Interface/AddOns")).unwrap();
        let game = Game::open(dir.path()).unwrap();
        (dir, game)
    }

    fn zip_of(path: &Path, files: &[(&str, &str)]) {
        let mut zip = zip::ZipWriter::new(fs::File::create(path).unwrap());
        for (name, body) in files {
            zip.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
            zip.write_all(body.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
    }

    #[test]
    fn unpacks_and_reads_folders() {
        let (dir, _) = game();
        let zip = dir.path().join("a.zip");
        zip_of(&zip, &[("Foo/Foo.toc", "## Interface: 16001\n## Title: Foo"), ("Foo/Foo.lua", ""), ("README.md", "")]);
        let (_, folders) = unpack(&zip, &dir.path().join("out")).unwrap();
        assert_eq!(folders.len(), 1);
        assert_eq!(folders[0].fit, Fit::Forever);
    }

    #[test]
    fn unwraps_one_extra_folder() {
        let (dir, _) = game();
        let zip = dir.path().join("a.zip");
        zip_of(&zip, &[("Foo-v1/Foo/Foo.toc", "## Interface: 16001"), ("Foo-v1/FooLib/FooLib.toc", "## Interface: 16001")]);
        let (base, folders) = unpack(&zip, &dir.path().join("out")).unwrap();
        assert!(base.ends_with("Foo-v1"));
        assert_eq!(folders.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["Foo", "FooLib"]);
    }

    #[test]
    fn refuses_paths_outside() {
        let (dir, _) = game();
        let zip = dir.path().join("a.zip");
        zip_of(&zip, &[("../evil/x.toc", "## Interface: 16001")]);
        assert!(unpack(&zip, &dir.path().join("out")).is_err());
        assert!(!dir.path().join("evil").exists());
    }

    #[test]
    fn swaps_backs_up_and_removes() {
        let (dir, game) = game();
        let addons = game.addons_dir();
        fs::create_dir_all(addons.join("Foo")).unwrap();
        fs::write(addons.join("Foo/old.txt"), "old").unwrap();
        let base = dir.path().join("new");
        fs::create_dir_all(base.join("Foo")).unwrap();
        fs::write(base.join("Foo/new.txt"), "new").unwrap();

        let mut state = State::default();
        place(&game, "foo", &["Foo".into()], &base, &mut state).unwrap();
        assert!(addons.join("Foo/new.txt").exists());
        assert!(game.interface_dir().join(BACKUP_DIR).join("Foo/old.txt").exists());

        state.addons.insert("foo".into(), Installed {
            name: "Foo".into(),
            source: Source::github("a/b"),
            tag: "v1".into(),
            asset: "Foo.zip".into(),
            folders: vec!["Foo".into()],
            installed_at: 0,
        });
        state.save(&game).unwrap();
        remove(&game, "foo").unwrap();
        assert!(!addons.join("Foo").exists());
        assert!(game.interface_dir().join(BACKUP_DIR).join("Foo/new.txt").exists());
        assert!(State::load(&game).unwrap().addons.is_empty());
    }
}
