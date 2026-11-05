//! The commands the page calls (`crates/ui/src/bridge.rs`). Most answer with the whole
//! library, so the page never merges partial updates. Installs, updates and removals run one
//! at a time; their steps stream over the `onProgress` channel, a failed addon's included, so
//! one bad download never stops the rest.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use gryphon_core::github::{Client, Latest};
use gryphon_core::graphics::{self, Change};
use gryphon_core::shared::{Graphics, Library, Progress, SettingsInfo, Source, Step};
use gryphon_core::state::State as Record;
use gryphon_core::wow::Game;
use gryphon_core::{catalog, install, library};
use tauri::ipc::Channel;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use crate::settings::Settings;

/// How long a check stands before an install or update asks GitHub again.
const FRESH: Duration = Duration::from_secs(10 * 60);
const NO_GAME: &str = "WoW Forever was not found. Choose its folder in Settings.";

type Answer<T> = Result<T, String>;

pub struct Kit {
    settings: Mutex<Settings>,
    checked: Mutex<Checked>,
    /// Held for the length of an install, update or removal.
    work: tokio::sync::Mutex<()>,
}

#[derive(Default)]
struct Checked {
    at: Option<Instant>,
    latest: BTreeMap<String, Result<Latest, String>>,
}

impl Kit {
    pub fn new(settings: Settings) -> Kit {
        Kit { settings: Mutex::new(settings), checked: Mutex::default(), work: tokio::sync::Mutex::new(()) }
    }

    /// The folder picked in Settings, or the usual places when there is none or it moved.
    fn game(&self) -> Option<Game> {
        // a folder picked by hand is the only one used: when it no longer opens (its drive
        // unplugged, say) the app says so rather than quietly working on another install
        match self.settings.lock().unwrap().wow_path.clone() {
            Some(path) => Game::open(&path),
            None => Game::find(),
        }
    }

    fn client(&self) -> Answer<Client> {
        let token = self.settings.lock().unwrap().github_token.clone().or_else(|| std::env::var("GITHUB_TOKEN").ok());
        Client::new(token).map_err(|e| e.to_string())
    }

    fn library(&self) -> Answer<Library> {
        let checks = self
            .checked
            .lock()
            .unwrap()
            .latest
            .iter()
            .map(|(id, latest)| (id.clone(), latest.as_ref().map(library::latest_info).map_err(Clone::clone)))
            .collect();
        library::library(self.game().as_ref(), &checks).map_err(|e| e.to_string())
    }

    /// Looks up every pick and everything installed, and keeps the answers.
    async fn check(&self, client: &Client, game: Option<&Game>) -> Answer<()> {
        let mut sources: Vec<(String, Source)> = catalog::CATALOG.iter().map(|e| (e.id.to_string(), e.source())).collect();
        if let Some(game) = game {
            let record = Record::load(game).map_err(|e| e.to_string())?;
            sources.extend(record.addons.into_iter().filter(|(id, _)| catalog::find(id).is_none()).map(|(id, a)| (id, a.source)));
        }
        let latest = library::check(client, sources).await.into_iter().map(|(id, r)| (id, r.map_err(|e| e.to_string()))).collect();
        *self.checked.lock().unwrap() = Checked { at: Some(Instant::now()), latest };
        Ok(())
    }

    fn fresh(&self) -> bool {
        self.checked.lock().unwrap().at.is_some_and(|at| at.elapsed() < FRESH)
    }

    fn cached(&self, id: &str) -> Option<Latest> {
        self.checked.lock().unwrap().latest.get(id).and_then(|r| r.as_ref().ok()).cloned()
    }

    /// The newest release of one addon: the last check's answer while it is fresh.
    async fn latest(&self, client: &Client, id: &str, source: &Source) -> Answer<Latest> {
        if let Some(latest) = self.cached(id).filter(|_| self.fresh()) {
            return Ok(latest);
        }
        let latest = client.latest(source).await.map_err(|e| e.to_string());
        self.checked.lock().unwrap().latest.insert(id.to_string(), latest.clone());
        latest
    }

    /// Installs one addon, sending its steps to the page. A failure is sent too, then returned.
    async fn install_one(&self, client: &Client, game: &Game, id: &str, name: &str, source: &Source, progress: &Channel<Progress>) -> Answer<()> {
        let mut send = |step: Step| {
            let _ = progress.send(Progress { id: id.to_string(), step });
        };
        send(Step::Checking);
        let outcome = match self.latest(client, id, source).await {
            Ok(latest) => install::install_release(client, game, id, name, source, &latest, &mut send).await.map(drop).map_err(|e| e.to_string()),
            Err(e) => Err(e),
        };
        if let Err(message) = &outcome {
            send(Step::Failed { message: message.clone() });
        }
        outcome
    }

    /// A pick, or an addon added from GitHub earlier.
    fn find(&self, game: &Game, id: &str) -> Answer<(String, Source)> {
        if let Some(entry) = catalog::find(id) {
            return Ok((entry.name.to_string(), entry.source()));
        }
        let record = Record::load(game).map_err(|e| e.to_string())?;
        record.addons.get(id).map(|a| (a.name.clone(), a.source.clone())).ok_or_else(|| format!("{id} is not a pick or an addon added here."))
    }

    fn update_settings(&self, app: &AppHandle, change: impl FnOnce(&mut Settings)) -> Answer<SettingsInfo> {
        let mut settings = self.settings.lock().unwrap();
        change(&mut settings);
        settings.save(app)?;
        Ok(settings.info())
    }
}

#[tauri::command]
pub fn library(kit: State<'_, Kit>) -> Answer<Library> {
    kit.library()
}

#[tauri::command]
pub async fn check_updates(kit: State<'_, Kit>) -> Answer<Library> {
    let client = kit.client()?;
    let game = kit.game();
    kit.check(&client, game.as_ref()).await?;
    kit.library()
}

/// Installs or reinstalls each id, and what a pick needs before it.
#[tauri::command]
pub async fn install(kit: State<'_, Kit>, ids: Vec<String>, on_progress: Channel<Progress>) -> Answer<Library> {
    let _work = kit.work.lock().await;
    let game = kit.game().ok_or(NO_GAME)?;
    let client = kit.client()?;
    for id in &ids {
        let record = Record::load(&game).map_err(|e| e.to_string())?;
        let mut ready = true;
        for need in catalog::find(id).map_or(&[][..], |e| e.needs) {
            if !record.addons.contains_key(*need) {
                let entry = catalog::find(need).expect("needs name catalog ids");
                ready &= kit.install_one(&client, &game, entry.id, entry.name, &entry.source(), &on_progress).await.is_ok();
            }
        }
        if ready {
            let (name, source) = kit.find(&game, id)?;
            let _ = kit.install_one(&client, &game, id, &name, &source, &on_progress).await;
        }
    }
    kit.library()
}

/// Installs an addon from any GitHub repo whose releases carry a Forever zip. A repo that is
/// one of the picks installs as that pick.
#[tauri::command]
pub async fn add_github(kit: State<'_, Kit>, repo: String, on_progress: Channel<Progress>) -> Answer<Library> {
    let source = Source::parse(&repo).ok_or("That is not a GitHub repo. Paste owner/name or a github.com link.")?;
    let _work = kit.work.lock().await;
    let game = kit.game().ok_or(NO_GAME)?;
    let client = kit.client()?;
    let _ = match catalog::for_source(&source) {
        Some(entry) => kit.install_one(&client, &game, entry.id, entry.name, &entry.source(), &on_progress).await,
        None => {
            let name = source.repo.rsplit('/').next().unwrap_or(&source.repo).to_string();
            kit.install_one(&client, &game, &library::id_for(&source), &name, &source, &on_progress).await
        }
    };
    kit.library()
}

/// Installs the newest release of everything installed that has one.
#[tauri::command]
pub async fn update_all(kit: State<'_, Kit>, on_progress: Channel<Progress>) -> Answer<Library> {
    let _work = kit.work.lock().await;
    let game = kit.game().ok_or(NO_GAME)?;
    let client = kit.client()?;
    if !kit.fresh() {
        kit.check(&client, Some(&game)).await?;
    }
    let record = Record::load(&game).map_err(|e| e.to_string())?;
    for (id, addon) in &record.addons {
        // an addon whose check failed shows why on its row
        if kit.cached(id).is_some_and(|latest| latest.tag != addon.tag) {
            let _ = kit.install_one(&client, &game, id, &addon.name, &addon.source, &on_progress).await;
        }
    }
    kit.library()
}

/// Takes an addon out; its folders move to the backup.
#[tauri::command]
pub async fn remove(kit: State<'_, Kit>, id: String) -> Answer<Library> {
    let _work = kit.work.lock().await;
    let game = kit.game().ok_or(NO_GAME)?;
    let record = Record::load(&game).map_err(|e| e.to_string())?;
    if let Some(user) = catalog::CATALOG.iter().find(|e| e.needs.contains(&id.as_str()) && record.addons.contains_key(e.id)) {
        return Err(format!("{} needs this one. Remove {} first.", user.name, user.name));
    }
    install::remove(&game, &id).map_err(|e| e.to_string())?;
    kit.library()
}

#[tauri::command]
pub async fn choose_game_folder(app: AppHandle, kit: State<'_, Kit>) -> Answer<Library> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog().file().set_title("Choose your World of Warcraft folder").pick_folder(move |picked| {
        let _ = tx.send(picked);
    });
    let Some(picked) = rx.await.ok().flatten() else {
        return kit.library();
    };
    let path = picked.into_path().map_err(|e| e.to_string())?;
    let game = Game::open(&path).ok_or_else(|| {
        format!("There is no WoW Forever install in {}. Choose the World of Warcraft folder, the one with _forever_ inside.", path.display())
    })?;
    kit.update_settings(&app, |s| s.wow_path = Some(game.root))?;
    kit.library()
}

/// Forgets the folder picked by hand and looks in the usual places again.
#[tauri::command]
pub fn auto_game_folder(app: AppHandle, kit: State<'_, Kit>) -> Answer<Library> {
    kit.update_settings(&app, |s| s.wow_path = None)?;
    kit.library()
}

/// Shows `addons`, `backup` or the game folder in the file manager.
#[tauri::command]
pub fn open_folder(kit: State<'_, Kit>, which: String) -> Answer<()> {
    let game = kit.game().ok_or(NO_GAME)?;
    let path = match which.as_str() {
        "addons" => game.addons_dir(),
        "backup" => game.interface_dir().join(install::BACKUP_DIR),
        _ => game.root,
    };
    if !path.exists() {
        return Err(if which == "backup" { "Nothing has been backed up yet." } else { "That folder is not there yet." }.into());
    }
    reveal(&path)
}

/// Opens an addon's GitHub page in the browser. Only a repo goes in, never a whole address.
#[tauri::command]
pub fn open_github(repo: String) -> Answer<()> {
    let source = Source::parse(&repo).ok_or_else(|| format!("{repo} is not a GitHub repo."))?;
    reveal(Path::new(&source.url()))
}

fn reveal(path: &Path) -> Answer<()> {
    #[cfg(target_os = "macos")]
    let opener = "open";
    #[cfg(target_os = "windows")]
    let opener = "explorer";
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let opener = "xdg-open";
    std::process::Command::new(opener).arg(path).spawn().map(drop).map_err(|e| format!("Could not open {}: {e}", path.display()))
}

/// The game settings Gryphon can change in Config.wtf, and whether the game is open.
#[tauri::command]
pub fn graphics(kit: State<'_, Kit>) -> Answer<Graphics> {
    let game = kit.game().ok_or(NO_GAME)?;
    graphics::read(&game).map_err(|e| e.to_string())
}

/// Changes settings in Config.wtf: a number sets one, `null` puts it back the way it was.
#[tauri::command]
pub fn set_graphics(kit: State<'_, Kit>, changes: BTreeMap<String, Option<i64>>) -> Answer<Graphics> {
    let game = kit.game().ok_or(NO_GAME)?;
    let changes: Vec<(String, Change)> = changes.into_iter().map(|(id, value)| (id, value.map_or(Change::PutBack, Change::Set))).collect();
    graphics::change(&game, &changes).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn settings(kit: State<'_, Kit>) -> SettingsInfo {
    kit.settings.lock().unwrap().info()
}

#[tauri::command]
pub fn set_light(app: AppHandle, kit: State<'_, Kit>, light: bool) -> Answer<SettingsInfo> {
    kit.update_settings(&app, |s| s.light = light)
}

/// Saves a GitHub token; an empty one removes it.
#[tauri::command]
pub fn set_token(app: AppHandle, kit: State<'_, Kit>, token: String) -> Answer<SettingsInfo> {
    let token = token.trim().to_string();
    kit.update_settings(&app, |s| s.github_token = (!token.is_empty()).then_some(token))
}
