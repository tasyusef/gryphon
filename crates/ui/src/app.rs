//! The window: the header (the wordmark, where you are, the clock), the sidebar, the screen
//! and the bottom bar. Toolbox's chrome (styles/chrome.css), with Gryphon's screens in it.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::time::Duration;

use gryphon_shared::{Library, Progress, SettingsInfo, Step};
use leptos::prelude::*;
use leptos::task::spawn_local;
use serde_json::{Value, json};

use crate::updates::Updates;
use crate::{bridge, marks, screens, text};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Picks,
    Github,
    Folders,
    Settings,
}

impl Screen {
    pub fn name(self) -> &'static str {
        match self {
            Screen::Picks => "Picks",
            Screen::Github => "GitHub",
            Screen::Folders => "Folders",
            Screen::Settings => "Settings",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Quiet,
    Done,
    Alert,
}

/// The page's state, shared through context. Every change to addons goes through the app and
/// comes back as a whole [`Library`].
#[derive(Clone, Copy)]
pub struct Kit {
    pub library: RwSignal<Option<Library>>,
    pub settings: RwSignal<SettingsInfo>,
    pub screen: RwSignal<Screen>,
    /// The addon whose own page is open over the screen's list.
    pub detail: RwSignal<Option<String>>,
    /// Where the list was scrolled to when that page opened.
    list_scroll: RwSignal<i32>,
    /// An install, update, removal or check is running; the actions wait.
    pub busy: RwSignal<bool>,
    pub checking: RwSignal<bool>,
    /// The step each addon is on while it installs.
    pub steps: RwSignal<BTreeMap<String, Step>>,
    /// Each addon's last failure, until it is tried again.
    pub failures: RwSignal<BTreeMap<String, String>>,
    /// The bottom bar's line.
    pub status: RwSignal<(String, Tone)>,
    /// The bottom bar's meter, 0 to 1, while a download runs.
    pub meter: RwSignal<Option<f64>>,
    /// The sidebar's row for updates to Gryphon itself.
    pub updates: Updates,
}

/// What one run of installs did, for the bottom bar's last word.
#[derive(Default)]
struct Tally {
    done: usize,
    failed: Vec<String>,
}

impl Kit {
    fn new() -> Kit {
        Kit {
            library: RwSignal::new(None),
            settings: RwSignal::new(SettingsInfo::default()),
            screen: RwSignal::new(Screen::Picks),
            detail: RwSignal::new(None),
            list_scroll: RwSignal::new(0),
            busy: RwSignal::new(false),
            checking: RwSignal::new(false),
            steps: RwSignal::new(BTreeMap::new()),
            failures: RwSignal::new(BTreeMap::new()),
            status: RwSignal::new((String::new(), Tone::Quiet)),
            meter: RwSignal::new(None),
            updates: Updates::new(),
        }
    }

    pub fn go(self, screen: Screen) {
        self.detail.set(None);
        self.screen.set(screen);
    }

    /// An addon's own page, from the top.
    pub fn open_addon(self, id: String) {
        self.list_scroll.set(screen_body().map_or(0, |b| b.scroll_top()));
        self.detail.set(Some(id));
        if let Some(body) = screen_body() {
            body.set_scroll_top(0);
        }
    }

    /// Back to the list, where it was.
    pub fn close_addon(self) {
        self.detail.set(None);
        let top = self.list_scroll.get_untracked();
        request_animation_frame(move || {
            if let Some(body) = screen_body() {
                body.set_scroll_top(top);
            }
        });
    }

    pub fn has_game(&self) -> bool {
        self.library.with(|l| l.as_ref().is_some_and(|l| l.game.is_some()))
    }

    fn name_of(&self, id: &str) -> String {
        self.library
            .with_untracked(|l| l.as_ref().and_then(|l| l.addons.iter().find(|a| a.id == id)).map(|a| a.name.clone()))
            .unwrap_or_else(|| id.to_string())
    }

    fn say(&self, line: impl Into<String>, tone: Tone) {
        self.status.set((line.into(), tone));
    }

    /// The library and settings, then a check of GitHub.
    fn load(self) {
        spawn_local(async move {
            if let Ok(settings) = bridge::call::<SettingsInfo>("settings", json!({}), None).await {
                self.settings.set(settings);
            }
            match bridge::call::<Library>("library", json!({}), None).await {
                Ok(library) => self.library.set(Some(library)),
                Err(e) => self.say(e, Tone::Alert),
            }
            self.check();
        });
    }

    pub fn check(self) {
        if self.busy.get_untracked() {
            return;
        }
        self.busy.set(true);
        self.checking.set(true);
        self.say("Checking GitHub for new releases", Tone::Quiet);
        spawn_local(async move {
            match bridge::call::<Library>("check_updates", json!({}), None).await {
                Ok(library) => {
                    let updates = library.addons.iter().filter(|a| a.update_available()).count();
                    let failed = library.addons.iter().filter(|a| a.problem.is_some()).count();
                    // the primary button already counts the updates, so a failure takes the line
                    let line = match (failed, updates) {
                        (0, 0) => "Everything is up to date.".to_string(),
                        (0, n) => format!("{} ready.", text::count(n, "update", "updates")),
                        (1, _) => "1 check failed. Its row says why.".to_string(),
                        (n, _) => format!("{n} checks failed. Their rows say why."),
                    };
                    self.library.set(Some(library));
                    self.say(line, Tone::Quiet);
                }
                Err(e) => self.say(e, Tone::Alert),
            }
            self.checking.set(false);
            self.busy.set(false);
        });
    }

    /// Runs one change through the app, one at a time: its steps on the rows and the bar, then
    /// the new library. `done` words the outcome when nothing failed.
    fn run(self, command: &'static str, args: Value, streams: bool, done: impl FnOnce(&Library, usize) -> String + 'static) {
        if self.busy.get_untracked() {
            return;
        }
        self.busy.set(true);
        let tally = Rc::new(RefCell::new(Tally::default()));
        let on_progress: Option<Box<dyn Fn(Progress)>> = streams.then(|| {
            let tally = tally.clone();
            Box::new(move |progress: Progress| self.step(progress, &tally)) as Box<dyn Fn(Progress)>
        });
        spawn_local(async move {
            let answer = bridge::call::<Library>(command, args, on_progress).await;
            self.steps.set(BTreeMap::new());
            self.meter.set(None);
            match answer {
                Ok(library) => {
                    let tally = tally.borrow();
                    match tally.failed.as_slice() {
                        [] => self.say(done(&library, tally.done), Tone::Done),
                        [only] => self.say(only.clone(), Tone::Alert),
                        many => self.say(format!("{} did not install. Their rows say why.", text::count(many.len(), "addon", "addons")), Tone::Alert),
                    }
                    self.library.set(Some(library));
                }
                Err(e) => self.say(e, Tone::Alert),
            }
            self.busy.set(false);
        });
    }

    fn step(self, progress: Progress, tally: &RefCell<Tally>) {
        let Progress { id, step } = progress;
        let name = self.name_of(&id);
        match &step {
            Step::Checking => {
                self.failures.update(|f| {
                    f.remove(&id);
                });
                self.meter.set(None);
                self.say(format!("{name}: finding the release"), Tone::Quiet);
            }
            Step::Downloading { done, total } => {
                self.meter.set(total.map(|t| *done as f64 / t.max(1) as f64));
                let line = match total {
                    Some(total) => format!("{name}: downloading {} of {}", text::size(*done), text::size(*total)),
                    None => format!("{name}: downloading {}", text::size(*done)),
                };
                self.say(line, Tone::Quiet);
            }
            Step::Unpacking => {
                self.meter.set(Some(1.0));
                self.say(format!("{name}: checking the files"), Tone::Quiet);
            }
            Step::Done => {
                tally.borrow_mut().done += 1;
                self.meter.set(None);
                self.say(format!("{name} installed"), Tone::Done);
            }
            Step::Failed { message } => {
                tally.borrow_mut().failed.push(message.clone());
                self.failures.update(|f| {
                    f.insert(id.clone(), message.clone());
                });
                self.meter.set(None);
                self.say(message.clone(), Tone::Alert);
            }
        }
        self.steps.update(|s| {
            s.insert(id, step);
        });
    }

    pub fn install(self, ids: Vec<String>) {
        let names: Vec<String> = ids.iter().map(|id| self.name_of(id)).collect();
        self.run("install", json!({ "ids": ids }), true, move |_, done| match names.as_slice() {
            [one] => format!("{one} installed. It loads next time you log in."),
            _ => format!("{} installed. They load next time you log in.", text::count(done, "addon", "addons")),
        });
    }

    pub fn add(self, repo: String) {
        self.run("add_github", json!({ "repo": repo }), true, |library, _| {
            let newest = library.addons.iter().filter_map(|a| Some((a.installed.as_ref()?.installed_at, &a.name))).max();
            match newest {
                Some((_, name)) => format!("{name} installed. It loads next time you log in."),
                None => "Installed.".to_string(),
            }
        });
    }

    pub fn update_all(self) {
        self.run("update_all", json!({}), true, |_, done| match done {
            0 => "Nothing needed updating.".to_string(),
            n => format!("{} updated. Type /reload in game.", text::count(n, "addon", "addons")),
        });
    }

    pub fn remove(self, id: String) {
        let name = self.name_of(&id);
        self.run("remove", json!({ "id": id }), false, move |_, _| format!("{name} removed. Its folders are in the backup."));
    }

    /// Calls something that answers with the library, outside the install queue.
    fn refresh(self, command: &'static str, args: Value) {
        spawn_local(async move {
            match bridge::call::<Library>(command, args, None).await {
                Ok(library) => {
                    let found = library.game.is_some();
                    self.library.set(Some(library));
                    if let Ok(settings) = bridge::call::<SettingsInfo>("settings", json!({}), None).await {
                        self.settings.set(settings);
                    }
                    self.say(if found { "Found WoW Forever." } else { "WoW Forever was not found in the usual places." }, Tone::Quiet);
                }
                Err(e) => self.say(e, Tone::Alert),
            }
        });
    }

    pub fn choose_folder(self) {
        self.refresh("choose_game_folder", json!({}));
    }

    pub fn auto_folder(self) {
        self.refresh("auto_game_folder", json!({}));
    }

    pub fn open(self, which: &'static str) {
        spawn_local(async move {
            if let Err(e) = bridge::call::<()>("open_folder", json!({ "which": which }), None).await {
                self.say(e, Tone::Alert);
            }
        });
    }

    pub fn open_github(self, repo: String) {
        spawn_local(async move {
            if let Err(e) = bridge::call::<()>("open_github", json!({ "repo": repo }), None).await {
                self.say(e, Tone::Alert);
            }
        });
    }

    pub fn set_light(self, light: bool) {
        self.settings.update(|s| s.light = light);
        spawn_local(async move {
            let _ = bridge::call::<SettingsInfo>("set_light", json!({ "light": light }), None).await;
        });
    }

    pub fn set_token(self, token: String) {
        let saving = !token.trim().is_empty();
        spawn_local(async move {
            match bridge::call::<SettingsInfo>("set_token", json!({ "token": token }), None).await {
                Ok(settings) => {
                    self.settings.set(settings);
                    self.say(if saving { "GitHub token saved." } else { "GitHub token removed." }, Tone::Done);
                }
                Err(e) => self.say(e, Tone::Alert),
            }
        });
    }
}

/// The scrolling column the screens sit in.
fn screen_body() -> Option<web_sys::Element> {
    document().query_selector(".screen-body").ok().flatten()
}

#[component]
pub fn App() -> impl IntoView {
    let kit = Kit::new();
    provide_context(kit);
    kit.load();

    let motion = window().match_media("(prefers-reduced-motion: no-preference)").ok().flatten().is_some_and(|m| m.matches());
    Effect::new(move |_| {
        let light = kit.settings.with(|s| s.light);
        if let Some(root) = document().document_element() {
            let _ = if light { root.set_attribute("data-theme", "light") } else { root.remove_attribute("data-theme") };
        }
    });

    let clock = RwSignal::new(String::new());
    let tick = move || {
        let now = js_sys::Date::new_0();
        clock.set(format!("{:02}:{:02}:{:02}", now.get_hours(), now.get_minutes(), now.get_seconds()));
    };
    tick();
    let _ = set_interval_with_handle(tick, Duration::from_secs(1));

    let route = move || {
        let place = if bridge::in_app() { "WoW Forever" } else { "Preview" };
        view! {
            <span class="dim">{place}</span>
            <span class="sep dim">"/"</span>
            <span>{move || kit.screen.get().name()}</span>
            {move || {
                let name = kit.detail.get().and_then(|id| kit.library.with(|l| l.as_ref()?.addons.iter().find(|a| a.id == id).map(|a| a.name.clone())));
                name.map(|name| view! {
                    <span class="sep dim">"/"</span>
                    <span>{name}</span>
                })
            }}
        }
    };

    view! {
        <div class="app" class:motion=motion>
            <header class="site-header">
                <button type="button" class="wordmark blackletter" aria-label="Gryphon: picks" on:click=move |_| kit.go(Screen::Picks)>
                    <span class="initial">"g"</span>
                    "ryphon"
                </button>
                " "
                <div class="readout lbl" aria-label="Status">
                    <span class="path" aria-hidden="true">
                        <span class="path-text">
                            <bdi>{route}</bdi>
                        </span>
                        <span class="cursor"></span>
                    </span>
                    " "
                    <span class="st-state sr-only" role="status">{move || kit.status.with(|(line, _)| line.clone())}</span>
                    " "
                    <span class="st-clock">{move || clock.get()}</span>
                </div>
            </header>
            " "
            <div class="app-body">
                <Sidebar />
                " "
                <main class="app-main">
                    <div class="scroll pad screen-body">
                        {move || match (kit.screen.get(), kit.detail.get()) {
                            (Screen::Picks | Screen::Github, Some(id)) => view! { <screens::AddonPage id /> }.into_any(),
                            (Screen::Picks, None) => view! { <screens::Picks /> }.into_any(),
                            (Screen::Github, None) => view! { <screens::Github /> }.into_any(),
                            (Screen::Folders, _) => view! { <screens::Folders /> }.into_any(),
                            (Screen::Settings, _) => view! { <screens::SettingsScreen /> }.into_any(),
                        }}
                    </div>
                    " "
                    <Toolbar />
                </main>
            </div>
        </div>
    }
}

#[component]
fn Sidebar() -> impl IntoView {
    let kit = expect_context::<Kit>();
    let counts = Memo::new(move |_| {
        kit.library.with(|l| {
            let Some(l) = l else { return (String::new(), String::new(), String::new()) };
            let picks: Vec<_> = l.addons.iter().filter(|a| a.featured).collect();
            let installed = picks.iter().filter(|a| a.installed.is_some()).count();
            let picks = format!("{installed}/{}", picks.len());
            let github = l.addons.iter().filter(|a| !a.featured).count();
            (picks, github.to_string(), l.other.len().to_string())
        })
    });
    let row = move |screen: Screen, meta: Signal<String>| {
        view! {
            <button
                type="button"
                class="side-row"
                aria-current=move || (kit.screen.get() == screen).then_some("page")
                on:click=move |_| kit.go(screen)
            >
                {marks::initial(screen).map(|(letter, tint)| view! { <marks::Mark letter tint /> })}
                <span class="t nowrap">{screen.name()}</span>
                <span class="t side-meta dim">{meta}</span>
            </button>
        }
    };
    let light = move || kit.settings.with(|s| s.light);
    let updates = kit.updates;
    let update_text = Memo::new(move |_| updates.state.with(|state| state.text()));

    view! {
        <aside class="sidebar lbl" aria-label="Navigation">
            <div class="side-scroll">
                <section class="side-section" aria-label="Addons">
                    <div class="side-label dim">"Addons"</div>
                    {row(Screen::Picks, Signal::derive(move || counts.get().0))}
                    {row(Screen::Github, Signal::derive(move || counts.get().1))}
                    {row(Screen::Folders, Signal::derive(move || counts.get().2))}
                </section>
            </div>
            " "
            <div class="side-foot">
                {row(Screen::Settings, Signal::derive(String::new))}
                " "
                <button type="button" class="side-row" aria-pressed=move || (!light()).to_string() on:click=move |_| kit.set_light(!light())>
                    <span class="t">"Dark mode"</span>
                    <span class="switch-mark" class:on=move || !light() aria-hidden="true"></span>
                </button>
                " "
                <button
                    type="button"
                    class="side-row"
                    title=move || update_text.get().1
                    aria-busy=move || updates.state.with(|state| state.busy()).to_string()
                    on:click=move |_| updates.click()
                >
                    <span class="t nowrap">{move || update_text.get().0}</span>
                </button>
            </div>
        </aside>
    }
}

/// The way to check left, what is happening in the middle, the primary right.
#[component]
fn Toolbar() -> impl IntoView {
    let kit = expect_context::<Kit>();
    let updates = Memo::new(move |_| kit.library.with(|l| l.as_ref().map_or(0, |l| l.addons.iter().filter(|a| a.update_available()).count())));
    let primary = move || {
        if kit.library.with(|l| l.is_some()) && !kit.has_game() {
            return view! {
                <button type="button" class="cta lbl" disabled=move || kit.busy.get() on:click=move |_| kit.choose_folder()>
                    "Choose game folder"
                </button>
            }
            .into_any();
        }
        let n = updates.get();
        let label = match n {
            0 if kit.checking.get() => "Checking".to_string(),
            0 => "Up to date".to_string(),
            n => format!("Update {}", text::count(n, "addon", "addons")),
        };
        view! {
            <button type="button" class="cta lbl" disabled=move || n == 0 || kit.busy.get() on:click=move |_| kit.update_all()>
                {label}
            </button>
        }
        .into_any()
    };
    let meter = move || {
        kit.meter.get().map(|fraction| {
            // whole steps of the 8px grid: 12 cells across the 96px bar
            let width = (fraction.clamp(0.0, 1.0) * 12.0).floor() * 8.0;
            view! {
                <span class="bar" aria-hidden="true">
                    <span style=format!("width:{width}px")></span>
                </span>
            }
        })
    };

    view! {
        <div class="toolbar has-status">
            <button type="button" class="btn lbl" disabled=move || kit.busy.get() on:click=move |_| kit.check()>
                "Check for updates"
            </button>
            <span class="status lbl">
                {meter}
                <span
                    class="msg"
                    class:failed=move || kit.status.with(|(_, tone)| *tone == Tone::Alert)
                    class:dim=move || kit.status.with(|(line, tone)| *tone == Tone::Quiet && !line.is_empty() && !kit.busy.get())
                    title=move || kit.status.with(|(line, _)| line.clone())
                >
                    {move || kit.status.with(|(line, _)| line.clone())}
                </span>
            </span>
            {primary}
        </div>
    }
}
