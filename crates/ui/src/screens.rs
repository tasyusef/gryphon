//! The screens: the picks, addons added from GitHub, CurseForge's search, the other folders in
//! AddOns, the game's graphics settings, and Gryphon's own settings. Each is a page head (a label, the title, one line, and where the game is) over its
//! rows.

use std::collections::BTreeMap;

use gryphon_shared::{Addon, Fit, Folder, Found, Graphics, Search, Step, Tweak};
use leptos::prelude::*;
use leptos::task::spawn_local;
use serde_json::json;

use crate::app::{Kit, Tone};
use crate::{bridge, text};

/// Downloads bigger than this ask once more before they start.
const BIG_MB: u64 = 500;

/// `game` adds the line saying which game the list is for.
#[component]
fn Head(kicker: &'static str, title: &'static str, #[prop(into)] note: Signal<String>, #[prop(optional)] game: bool) -> impl IntoView {
    view! {
        <header class="page-head">
            <p class="lbl kicker">{kicker}</p>
            <h1 class="display">{title}</h1>
            <p class="lbl note dim">{note}</p>
            {game.then(|| view! { <GameLine /> })}
        </header>
    }
}

/// The game this works on, or the warning that there is none.
#[component]
fn GameLine() -> impl IntoView {
    let kit = expect_context::<Kit>();
    move || {
        kit.library.with(|l| match l.as_ref().map(|l| l.game.clone()) {
            None => ().into_any(),
            Some(Some(game)) => {
                let build = game.version.map(|v| format!("WoW Forever {v}")).unwrap_or_else(|| "WoW Forever".into());
                let root = game.root.clone();
                view! {
                    <p class="facts lbl note">
                        <span>{build}</span>
                        <span class="dim nowrap selectable" title=root>{game.root}</span>
                    </p>
                }
                .into_any()
            }
            Some(None) => view! {
                <p class="notice warn lbl note">"WoW Forever was not found. Choose its folder to install addons."</p>
            }
            .into_any(),
        })
    }
}

#[component]
pub fn Picks() -> impl IntoView {
    let kit = expect_context::<Kit>();
    let rows = move || {
        kit.library.with(|l| {
            l.as_ref().map(|l| l.addons.iter().filter(|a| a.featured).cloned().map(|addon| view! { <AddonRow addon /> }).collect_view())
        })
    };
    view! {
        <Head kicker="Picked by hand" title="Picks" note="Checked to have WoW Forever builds on GitHub. Each installs from its own releases.".to_string() game=true />
        <div class="addons">{rows}</div>
    }
}

#[component]
pub fn Github() -> impl IntoView {
    let kit = expect_context::<Kit>();
    let repo = RwSignal::new(String::new());
    let rows = move || {
        let added: Vec<Addon> = kit.library.with(|l| {
            l.as_ref().map(|l| l.addons.iter().filter(|a| !a.featured && a.source.curseforge.is_none()).cloned().collect()).unwrap_or_default()
        });
        if added.is_empty() {
            return view! { <p class="lbl note dim empty-note">"Nothing added yet."</p> }.into_any();
        }
        added.into_iter().map(|addon| view! { <AddonRow addon /> }).collect_view().into_any()
    };
    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let wanted = repo.get_untracked();
        if !wanted.trim().is_empty() {
            kit.add(wanted);
            repo.set(String::new());
        }
    };
    view! {
        <Head
            kicker="Any addon"
            title="From GitHub"
            note="Paste an addon's GitHub repo. It installs when a release has a zip built for WoW Forever, and nothing in AddOns changes until that is checked.".to_string()
            game=true
        />
        <form class="add-row" on:submit=submit>
            <input
                class="field body"
                type="text"
                spellcheck="false"
                autocomplete="off"
                placeholder="owner/name or a github.com link"
                aria-label="GitHub repo"
                prop:value=move || repo.get()
                on:input=move |ev| repo.set(event_target_value(&ev))
            />
            <button type="submit" class="btn lbl" disabled=move || kit.busy.get() || !kit.has_game() || repo.with(|r| r.trim().is_empty())>
                "Add"
            </button>
        </form>
        <div class="addons">{rows}</div>
    }
}

/// CurseForge: what was installed from it, then a search of the addons with a WoW Forever file.
/// It opens on the most popular. Without the player's own key, it asks for one.
#[component]
pub fn Curse() -> impl IntoView {
    let kit = expect_context::<Kit>();
    let has_key = move || kit.settings.with(|s| s.has_curseforge_key);
    view! {
        <Head
            kicker="Search"
            title="CurseForge"
            note="Addons on CurseForge with a WoW Forever build. They install and update like the rest, and nothing in AddOns changes until the download is checked.".to_string()
            game=true
        />
        {move || {
            if has_key() {
                view! { <CurseSearch /> }.into_any()
            } else {
                view! {
                    <section class="set">
                        <h2 class="lbl dim">"Your CurseForge key"</h2>
                        <p class="lbl note">
                            "CurseForge only answers apps that send a key, and each player brings their own. It's free: sign in at "
                            <span class="selectable">"console.curseforge.com"</span>
                            ", open API keys, and paste yours here."
                        </p>
                        <CurseKey />
                    </section>
                }
                .into_any()
            }
        }}
    }
}

/// The installed CurseForge addons, the search box and its results.
#[component]
fn CurseSearch() -> impl IntoView {
    let kit = expect_context::<Kit>();
    let words = RwSignal::new(String::new());
    let results = RwSignal::new(None::<Search>);
    let searched = RwSignal::new(String::new());
    let searching = RwSignal::new(false);

    let search = move |wanted: String| {
        if searching.get_untracked() {
            return;
        }
        searching.set(true);
        kit.say(if wanted.trim().is_empty() { "Loading CurseForge's most popular".to_string() } else { format!("Searching CurseForge for {}", wanted.trim()) }, Tone::Quiet);
        spawn_local(async move {
            match bridge::call::<Search>("search_curseforge", json!({ "words": wanted }), None).await {
                Ok(found) => {
                    let line = match found.results.len() {
                        0 => "Nothing on CurseForge matches with a WoW Forever build.".to_string(),
                        n => format!("{} with a WoW Forever build.", text::count(n, "addon", "addons")),
                    };
                    kit.say(line, Tone::Quiet);
                    searched.set(wanted.trim().to_string());
                    results.set(Some(found));
                }
                Err(e) => kit.say(e, Tone::Alert),
            }
            searching.set(false);
        });
    };
    search(String::new());

    let installed = move || {
        let added: Vec<Addon> =
            kit.library.with(|l| l.as_ref().map(|l| l.addons.iter().filter(|a| a.source.curseforge.is_some()).cloned().collect()).unwrap_or_default());
        (!added.is_empty()).then(|| {
            view! {
                <h2 class="lbl dim list-label">"Installed from CurseForge"</h2>
                <div class="addons">{added.into_iter().map(|addon| view! { <AddonRow addon /> }).collect_view()}</div>
            }
        })
    };
    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        search(words.get_untracked());
    };
    let heading = move || {
        let wanted = searched.get();
        if wanted.is_empty() { "Most popular".to_string() } else { format!("Matching {wanted}") }
    };
    let more = move || {
        results.with(|r| {
            let r = r.as_ref()?;
            let total = r.total.filter(|t| *t > r.results.len() as u64)?;
            Some(view! { <p class="lbl note dim empty-note">{format!("The first {} of {total}. More words narrow it down.", r.results.len())}</p> })
        })
    };
    let rows = move || {
        results.with(|r| match r {
            None => ().into_any(),
            Some(r) if r.results.is_empty() => view! { <p class="lbl note dim empty-note">"No addons with a WoW Forever build match."</p> }.into_any(),
            Some(r) => r.results.iter().cloned().map(|found| view! { <FoundRow found /> }).collect_view().into_any(),
        })
    };

    view! {
        {installed}
        <form class="add-row" on:submit=submit role="search">
            <input
                class="field body"
                type="search"
                spellcheck="false"
                autocomplete="off"
                placeholder="Search CurseForge"
                aria-label="Search CurseForge"
                prop:value=move || words.get()
                on:input=move |ev| words.set(event_target_value(&ev))
            />
            <button type="submit" class="btn lbl" disabled=move || searching.get()>
                {move || if searching.get() { "Searching" } else { "Search" }}
            </button>
        </form>
        <h2 class="lbl dim list-label">{heading}</h2>
        <div class="addons">{rows}</div>
        {more}
    }
}

/// One search result: its name, what it does and who made it, how popular and how recent it
/// is, and Install, or why it can't be.
#[component]
fn FoundRow(found: Found) -> impl IntoView {
    let kit = expect_context::<Kit>();
    let Found { id, slug, name, summary, authors, downloads, updated, distributable, installed_as, .. } = found;
    let library_id = format!("curseforge:{id}");
    let step = {
        let library_id = library_id.clone();
        Memo::new(move |_| kit.steps.with(|s| s.get(&library_id).cloned()))
    };
    let failure = {
        let library_id = library_id.clone();
        Memo::new(move |_| kit.failures.with(|f| f.get(&library_id).cloned()))
    };
    // installed since the search ran, or already when it did
    let installed = {
        let library_id = library_id.clone();
        Memo::new(move |_| {
            kit.library
                .with(|l| l.as_ref()?.addons.iter().find(|a| a.id == library_id && a.installed.is_some()).map(|a| (a.id.clone(), a.name.clone())))
                .or_else(|| installed_as.clone())
        })
    };
    let by = (!authors.is_empty()).then(|| format!("By {}", authors.join(", ")));
    let updated = updated.as_deref().map(text::date);
    let page = slug.clone();
    let title = name.clone();

    let act = move || {
        if let Some(step) = step.get() {
            return view! { <Meter step /> }.into_any();
        }
        match installed.get() {
            Some((other, shown)) => {
                let line = if other == library_id { "Installed".to_string() } else { format!("Installed as {shown}") };
                view! { <span class="lbl dim">{line}</span> }.into_any()
            }
            None if !distributable => view! {
                <span class="lbl dim" title="Its author only lets CurseForge's own app download it.">"CurseForge app only"</span>
            }
            .into_any(),
            None => {
                let (slug, name) = (slug.clone(), name.clone());
                view! {
                    <button
                        type="button"
                        class="btn lbl"
                        disabled=move || kit.busy.get() || !kit.has_game()
                        on:click=move |_| kit.add_curse(id, slug.clone(), name.clone())
                    >
                        "Install"
                    </button>
                }
                .into_any()
            }
        }
    };

    view! {
        <article class="addon found reveal-scope">
            <div class="what">
                <h2 class="body">{title}</h2>
                <p class="lbl note dim">{summary}</p>
                {by.map(|line| view! { <p class="lbl note dim">{line}</p> })}
                {move || failure.get().map(|message| view! { <p class="lbl note failed selectable">{message}</p> })}
            </div>
            <div class="facts">
                <span class="lbl note nowrap">{format!("{} downloads", text::downloads(downloads))}</span>
                {updated.map(|date| view! { <span class="lbl dim nowrap">{date}</span> })}
            </div>
            <div class="act">
                {act}
                <button type="button" class="btn lbl reveal-item" on:click=move |_| kit.open_curseforge(page.clone())>"Page"</button>
            </div>
        </article>
    }
}

/// The box for the player's CurseForge key.
#[component]
fn CurseKey() -> impl IntoView {
    let kit = expect_context::<Kit>();
    let key = RwSignal::new(String::new());
    let save = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        kit.set_curseforge_key(key.get_untracked());
        key.set(String::new());
    };
    view! {
        <form class="add-row" on:submit=save>
            <input
                class="field body"
                type="password"
                autocomplete="off"
                placeholder="Your CurseForge API key"
                aria-label="CurseForge API key"
                prop:value=move || key.get()
                on:input=move |ev| key.set(event_target_value(&ev))
            />
            <button type="submit" class="btn lbl" disabled=move || key.with(|k| k.trim().is_empty())>"Save"</button>
        </form>
    }
}

/// The size an addon downloads at: the last check's, else the catalog's guess.
fn size_of(addon: &Addon) -> Option<u64> {
    addon.latest.as_ref().map(|l| l.size).or(addon.size_hint_mb.map(|mb| u64::from(mb) << 20))
}

/// An addon's version, the newer one when there is one, and where it stands.
struct Standing {
    now: String,
    then: Option<String>,
    state: Option<String>,
}

fn standing(addon: &Addon) -> Standing {
    let update = addon.update_available();
    let version = |tag: &str| text::shown_version(tag, &addon.source);
    let (now, then) = match (&addon.installed, &addon.latest) {
        (Some(have), Some(latest)) if update => (version(&have.tag), Some(format!("New {}", version(&latest.tag)))),
        (Some(have), _) => (version(&have.tag), None),
        (None, Some(latest)) => (version(&latest.tag), None),
        (None, None) => (String::new(), None),
    };
    let state = match (&addon.installed, &addon.latest) {
        (Some(_), Some(_)) if update => None,
        (Some(_), Some(_)) => Some("Up to date".to_string()),
        (Some(_), None) => Some("Installed".to_string()),
        (None, _) => Some(size_of(addon).map(text::size).unwrap_or_default()),
    };
    Standing { now, then, state }
}

/// The names of the addons this one installs first. `waiting` keeps only those not installed.
fn needs_names(kit: Kit, addon: &Addon, waiting: bool) -> Vec<String> {
    kit.library.with_untracked(|l| {
        let addons = l.as_ref().map(|l| l.addons.as_slice()).unwrap_or_default();
        addon
            .needs
            .iter()
            .filter_map(|need| match addons.iter().find(|a| &a.id == need) {
                Some(a) if waiting && a.installed.is_some() => None,
                Some(a) => Some(a.name.clone()),
                None => Some(need.clone()),
            })
            .collect()
    })
}

/// One addon: its name and what it does, its version, and what can be done with it. The name
/// opens its own page, and so does a click anywhere else on the row.
#[component]
fn AddonRow(addon: Addon) -> impl IntoView {
    let kit = expect_context::<Kit>();
    let id = addon.id.clone();
    let failure = {
        let id = id.clone();
        Memo::new(move |_| kit.failures.with(|f| f.get(&id).cloned()))
    };
    let Standing { now, then, state } = standing(&addon);
    let needs = needs_names(kit, &addon, true);
    let problem = addon.problem.clone();
    let blurb = addon.blurb.clone().unwrap_or_else(|| addon.source.label());
    let name = addon.name.clone();

    view! {
        <article class="addon reveal-scope">
            <div class="what">
                <h2 class="body">
                    <button type="button" class="open" on:click=move |_| kit.open_addon(id.clone())>{name}</button>
                </h2>
                <p class="lbl note dim">{blurb}</p>
                {(!needs.is_empty()).then(|| view! { <p class="lbl note dim">{format!("Installs {} first.", needs.join(", "))}</p> })}
                {move || {
                    failure.get().or_else(|| problem.clone()).map(|message| view! { <p class="lbl note failed selectable">{message}</p> })
                }}
            </div>
            <div class="facts">
                <span class="lbl note nowrap">{now}</span>
                {then.map(|line| view! { <span class="lbl note new nowrap">{line}</span> })}
                {state.map(|line| view! { <span class="lbl dim nowrap">{line}</span> })}
            </div>
            <div class="act">
                <Actions addon reveal=true />
            </div>
        </article>
    }
}

/// What can be done with an addon: install it, or update and remove it. While it installs, they
/// give way to its meter. `reveal` keeps Remove out of sight until its row is hovered.
#[component]
fn Actions(addon: Addon, #[prop(optional)] reveal: bool) -> impl IntoView {
    let kit = expect_context::<Kit>();
    let id = addon.id.clone();
    let step = {
        let id = id.clone();
        Memo::new(move |_| kit.steps.with(|s| s.get(&id).cloned()))
    };
    let armed = RwSignal::new(false);
    let update = addon.update_available();
    let size = size_of(&addon);
    let big = size.is_some_and(|s| s > BIG_MB << 20);
    let installed = addon.installed.is_some();
    let size_line = size.map(text::size).unwrap_or_default();

    move || {
        if let Some(step) = step.get() {
            return view! { <Meter step /> }.into_any();
        }
        let locked = move || kit.busy.get() || !kit.has_game();
        if !installed {
            let id = id.clone();
            let label = if armed.get() { format!("Get {size_line}?") } else { "Install".to_string() };
            return view! {
                <button
                    type="button"
                    class="btn lbl"
                    disabled=locked
                    on:blur=move |_| armed.set(false)
                    on:click=move |_| {
                        if big && !armed.get_untracked() {
                            armed.set(true);
                        } else {
                            armed.set(false);
                            kit.install(vec![id.clone()]);
                        }
                    }
                >
                    {label}
                </button>
            }
            .into_any();
        }
        let (update_id, remove_id) = (id.clone(), id.clone());
        view! {
            {update.then(|| view! {
                <button type="button" class="btn lbl" disabled=locked on:click=move |_| kit.install(vec![update_id.clone()])>"Update"</button>
            })}
            <button type="button" class="btn lbl" class:reveal-item=reveal disabled=locked on:click=move |_| kit.remove(remove_id.clone())>"Remove"</button>
        }
        .into_any()
    }
}

/// An addon's own page: what it does, the facts about its release, and the same actions as its
/// row. Escape or the label over the title goes back to the list.
#[component]
pub fn AddonPage(id: String) -> impl IntoView {
    let kit = expect_context::<Kit>();
    let escape = window_event_listener(leptos::ev::keydown, move |ev| {
        if ev.key() == "Escape" {
            kit.close_addon();
        }
    });
    on_cleanup(move || escape.remove());
    move || {
        let addon = kit.library.with(|l| l.as_ref().and_then(|l| l.addons.iter().find(|a| a.id == id).cloned()));
        match addon {
            Some(addon) => view! { <AddonDetail addon /> }.into_any(),
            // removed while its page was open: an addon from GitHub leaves the list with it
            None => {
                request_animation_frame(move || kit.close_addon());
                ().into_any()
            }
        }
    }
}

#[component]
fn AddonDetail(addon: Addon) -> impl IntoView {
    let kit = expect_context::<Kit>();
    let failure = {
        let id = addon.id.clone();
        Memo::new(move |_| kit.failures.with(|f| f.get(&id).cloned()))
    };
    let curse = addon.source.curseforge.clone();
    let back = match (addon.featured, &curse) {
        (true, _) => "Picks",
        (false, Some(_)) => "CurseForge",
        (false, None) => "From GitHub",
    };
    let Standing { now, then, .. } = standing(&addon);
    let size = size_of(&addon).map(text::size);
    let released = addon.latest.as_ref().and_then(|l| l.published_at.as_deref()).map(text::date);
    let folders = addon.installed.as_ref().map(|i| i.folders.join(", ")).filter(|f| !f.is_empty());
    let needs = needs_names(kit, &addon, false);
    let problem = addon.problem.clone();
    let blurb = addon.blurb.clone().unwrap_or_else(|| addon.source.label());
    let repo = addon.source.repo.clone();
    let source = addon.source.label();
    let about = addon.about.clone();
    let state = match (&addon.installed, addon.update_available()) {
        (Some(_), true) => "Installed, with an update waiting",
        (Some(_), false) => "Installed",
        (None, _) => "Not installed",
    };

    view! {
        <header class="page-head">
            <button type="button" class="back lbl dim" on:click=move |_| kit.close_addon()>
                <span class="mono-s" aria-hidden="true">"<"</span>
                {back}
            </button>
            <div class="head-row">
                <h1 class="display">{addon.name.clone()}</h1>
                <div class="head-actions">
                    <Actions addon=addon.clone() />
                    {match curse {
                        Some(project) => view! {
                            <button type="button" class="btn lbl" on:click=move |_| kit.open_curseforge(project.slug.clone())>"Open on CurseForge"</button>
                        }
                        .into_any(),
                        None => view! {
                            <button type="button" class="btn lbl" on:click=move |_| kit.open_github(repo.clone())>"Open on GitHub"</button>
                        }
                        .into_any(),
                    }}
                </div>
            </div>
            <p class="lbl note dim">{blurb}</p>
        </header>
        {move || failure.get().or_else(|| problem.clone()).map(|message| view! { <p class="lbl note failed selectable page-failed">{message}</p> })}
        {(!about.is_empty()).then(|| view! {
            <section class="part">
                <h2 class="display-s">"What it does"</h2>
                <ul class="points">
                    {about.into_iter().map(|point| view! { <li class="lbl note">{point}</li> }).collect_view()}
                </ul>
            </section>
        })}
        <section class="part">
            <h2 class="display-s">"Release"</h2>
            <dl class="spec">
                <dt class="lbl dim">"State"</dt>
                <dd class="lbl note">{state}</dd>
                {(!now.is_empty()).then(|| view! {
                    <dt class="lbl dim">"Version"</dt>
                    <dd class="lbl note">{now}{then.map(|line| view! { <span class="new">{line}</span> })}</dd>
                })}
                {size.map(|size| view! {
                    <dt class="lbl dim">"Download"</dt>
                    <dd class="lbl note">{size}</dd>
                })}
                {released.map(|date| view! {
                    <dt class="lbl dim">"Released"</dt>
                    <dd class="lbl note">{date}</dd>
                })}
                {(!needs.is_empty()).then(|| view! {
                    <dt class="lbl dim">"Installs first"</dt>
                    <dd class="lbl note">{needs.join(", ")}</dd>
                })}
                {folders.map(|folders| view! {
                    <dt class="lbl dim">"In AddOns"</dt>
                    <dd class="lbl note selectable">{folders}</dd>
                })}
                <dt class="lbl dim">"Source"</dt>
                <dd class="lbl note selectable">{source}</dd>
            </dl>
        </section>
    }
}

/// An install's progress in its row: the stepped bar while it downloads, else the step's name.
#[component]
fn Meter(step: Step) -> impl IntoView {
    let (fraction, label) = match &step {
        Step::Checking => (None, "Finding".to_string()),
        Step::Downloading { done, total: Some(total) } => {
            let fraction = *done as f64 / (*total).max(1) as f64;
            (Some(fraction), format!("{}%", (fraction * 100.0).floor()))
        }
        Step::Downloading { done, total: None } => (None, text::size(*done)),
        Step::Unpacking => (Some(1.0), "Checking".to_string()),
        Step::Done => (Some(1.0), "Done".to_string()),
        Step::Failed { .. } => (None, "Failed".to_string()),
    };
    // whole 8px cells of the 112px meter
    let width = fraction.map(|f| (f.clamp(0.0, 1.0) * 14.0).floor() * 8.0);
    view! {
        <span class="meter-cell">
            <span class="meter" aria-hidden="true">
                <span style=format!("width:{}px", width.unwrap_or(0.0))></span>
            </span>
            <span class="lbl">{label}</span>
        </span>
    }
}

#[component]
pub fn Folders() -> impl IntoView {
    let kit = expect_context::<Kit>();
    let rows = move || {
        let folders: Vec<Folder> = kit.library.with(|l| l.as_ref().map(|l| l.other.clone()).unwrap_or_default());
        if folders.is_empty() {
            return view! { <p class="lbl note dim empty-note">"Every folder in AddOns came from Gryphon."</p> }.into_any();
        }
        folders.into_iter().map(|folder| view! { <FolderRow folder /> }).collect_view().into_any()
    };
    view! {
        <Head
            kicker="Already in AddOns"
            title="Other folders"
            note="Folders Gryphon did not install, and whether WoW Forever will load them. It leaves these alone.".to_string()
            game=true
        />
        <div class="folders">{rows}</div>
    }
}

#[component]
fn FolderRow(folder: Folder) -> impl IntoView {
    let (fit, warn) = match folder.fit {
        Fit::Forever => ("Loads on Forever", false),
        Fit::OtherClient => ("Built for another client", true),
        Fit::Blocked => ("Turned off for Forever", true),
        Fit::NoToc => ("Not an addon", false),
    };
    let numbers = folder.interface.iter().map(u32::to_string).collect::<Vec<_>>().join(", ");
    let title = folder.title.filter(|t| t != &folder.name);
    view! {
        <article class="folder">
            <div class="what">
                <h2 class="body nowrap selectable">{folder.name}</h2>
                {title.map(|t| view! { <p class="lbl note dim">{t}</p> })}
            </div>
            <span class="lbl" class:warn=warn class:dim=folder.fit == Fit::NoToc>{fit}</span>
            <span class="lbl dim">{numbers}</span>
        </article>
    }
}

/// A section's label with a ? beside it that holds the longer story: shown on hover or focus,
/// and read out as the ?'s description.
#[component]
fn SetHead(#[prop(into)] title: String, #[prop(into)] id: String, #[prop(into)] tip: String) -> impl IntoView {
    let label = format!("About {}", title.to_lowercase());
    view! {
        <div class="set-head">
            <h2 class="lbl dim">{title}</h2>
            <span class="tip">
                <button type="button" class="tip-mark lbl" aria-label=label aria-describedby=id.clone()>
                    "?"
                </button>
                <span class="tip-body lbl note" role="tooltip" id=id>{tip}</span>
            </span>
        </div>
    }
}

/// The game's own settings that its graphics menu stops short of, kept in Config.wtf. Edits wait
/// for Save; putting one back is right away. Coming back to Gryphon after closing the game
/// checks again whether it's open.
#[component]
pub fn GraphicsScreen() -> impl IntoView {
    let kit = expect_context::<Kit>();
    let graphics = RwSignal::new(None::<Graphics>);
    let edits = RwSignal::new(BTreeMap::<String, i64>::new());
    let saving = RwSignal::new(false);

    let load = move || {
        spawn_local(async move {
            match bridge::call::<Graphics>("graphics", json!({}), None).await {
                Ok(loaded) => graphics.set(Some(loaded)),
                Err(e) => kit.say(e, Tone::Alert),
            }
        })
    };
    Effect::new(move |_| {
        if kit.has_game() {
            load();
        }
    });
    let focus = window_event_listener(leptos::ev::focus, move |_| {
        if kit.has_game() && !saving.get_untracked() {
            load();
        }
    });
    on_cleanup(move || focus.remove());

    let send = move |changes: BTreeMap<String, Option<i64>>, done: &'static str| {
        saving.set(true);
        spawn_local(async move {
            match bridge::call::<Graphics>("set_graphics", json!({ "changes": changes }), None).await {
                Ok(saved) => {
                    edits.update(|e| e.retain(|id, _| !changes.contains_key(id)));
                    graphics.set(Some(saved));
                    kit.say(done, Tone::Done);
                }
                Err(e) => kit.say(e, Tone::Alert),
            }
            saving.set(false);
        });
    };
    let locked = Signal::derive(move || saving.get() || graphics.with(|g| g.as_ref().is_none_or(|g| g.running || g.missing)));
    let out_of_range = move || {
        graphics.with(|g| {
            let tweaks = g.as_ref().map(|g| g.tweaks.as_slice()).unwrap_or_default();
            edits.with(|e| e.iter().any(|(id, v)| tweaks.iter().any(|t| &t.id == id && !(t.min..=t.max).contains(v))))
        })
    };
    let save = move |_| {
        let changes = edits.get_untracked().into_iter().map(|(id, v)| (id, Some(v))).collect();
        send(changes, "Saved. The changes take effect next time WoW Forever starts.");
    };

    let notice = move || {
        graphics.with(|g| match g {
            Some(g) if g.missing => Some("WoW Forever hasn't saved its settings yet. Start it once and close it, then come back."),
            Some(g) if g.running => Some("WoW Forever is open. Close it to change these, since it saves its own settings when it closes."),
            _ => None,
        })
        .map(|line| view! { <p class="notice warn lbl note">{line}</p> })
    };
    // built again only when the settings come back from the app, so typing keeps its place
    let rows = move || {
        graphics.with(|g| {
            g.as_ref().map(|g| {
                g.tweaks
                    .iter()
                    .cloned()
                    .map(|tweak| {
                        let put_back = move |id: String| send(BTreeMap::from([(id, None)]), "Put back. It takes effect next time WoW Forever starts.");
                        tweak_row(tweak, edits, locked, put_back)
                    })
                    .collect_view()
            })
        })
    };

    view! {
        <Head
            kicker="In the game"
            title="Graphics"
            note="Settings the game's own graphics menu stops short of. Gryphon changes them while the game is closed, and they take effect next time it starts.".to_string()
            game=true
        />
        {notice}
        {rows}
        {move || graphics.with(|g| g.is_some()).then(|| view! {
            <div class="set-actions tweak-save">
                <button
                    type="button"
                    class="cta lbl"
                    disabled=move || locked.get() || edits.with(|e| e.is_empty()) || out_of_range()
                    on:click=save
                >
                    "Save changes"
                </button>
                <span class="lbl note dim">"Config.wtf is copied to the backups first."</span>
            </div>
        })}
    }
}

/// One setting: its name and the longer story, what it does, then its number or switch, its
/// range, and the way back to the game's own value once Gryphon has changed it.
fn tweak_row(tweak: Tweak, edits: RwSignal<BTreeMap<String, i64>>, locked: Signal<bool>, put_back: impl Fn(String) + Copy + 'static) -> impl IntoView {
    let Tweak { id, name, about, tip, min, max, toggle, value, before, .. } = tweak;
    let shown = |n: i64| if toggle { if n == 1 { "on".to_string() } else { "off".to_string() } } else { n.to_string() };
    let edit = {
        let id = id.clone();
        move |n: Option<i64>| {
            edits.update(|e| match n {
                Some(n) if n != value => {
                    e.insert(id.clone(), n);
                }
                _ => {
                    e.remove(&id);
                }
            })
        }
    };
    let current = {
        let id = id.clone();
        Memo::new(move |_| edits.with(|e| e.get(&id).copied()).unwrap_or(value))
    };
    let control = if toggle {
        let on = move || current.get() == 1;
        view! {
            <button
                type="button"
                class="btn lbl"
                aria-pressed=move || on().to_string()
                aria-label=name.clone()
                disabled=move || locked.get()
                on:click=move |_| edit(Some(if on() { 0 } else { 1 }))
            >
                <span>{move || if on() { "On" } else { "Off" }}</span>
                <span class="switch-mark" class:on=on aria-hidden="true"></span>
            </button>
        }
        .into_any()
    } else {
        view! {
            <input
                class="field body"
                type="number"
                inputmode="numeric"
                min=min
                max=max
                step="1"
                aria-label=name.clone()
                value=value.to_string()
                disabled=move || locked.get()
                on:input=move |ev| edit(event_target_value(&ev).trim().parse().ok())
            />
            <span class="lbl dim" class:failed=move || !(min..=max).contains(&current.get())>{format!("{min} to {max}")}</span>
        }
        .into_any()
    };
    let back = before.map(|before| {
        let id = id.clone();
        view! {
            <button type="button" class="btn lbl" disabled=move || locked.get() on:click=move |_| put_back(id.clone())>
                {format!("Put back {}", shown(before))}
            </button>
        }
    });
    view! {
        <section class="set">
            <SetHead title=name.clone() id=format!("tip-{}", id.to_lowercase()) tip />
            <p class="lbl note dim">{about}</p>
            <div class="tweak">
                {control}
                {back}
            </div>
        </section>
    }
}

#[component]
pub fn SettingsScreen() -> impl IntoView {
    let kit = expect_context::<Kit>();
    let token = RwSignal::new(String::new());
    let game_root = move || kit.library.with(|l| l.as_ref().and_then(|l| l.game.as_ref()).map(|g| g.root.clone()));
    let chosen = move || kit.settings.with(|s| s.wow_path.is_some());
    let has_game = move || kit.has_game();

    let save = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        kit.set_token(token.get_untracked());
        token.set(String::new());
    };

    view! {
        <Head kicker="Gryphon" title="Settings" note="Saved on this computer.".to_string() />
        <section class="set">
            <SetHead
                title="Game folder"
                id="tip-game"
                tip="The World of Warcraft folder, the one with _forever_ inside. Gryphon looks in the usual places. If yours is somewhere else, choose it here. If a folder you chose stops opening, plug its drive back in or choose it again."
            />
            <p class="body selectable path-line">
                {move || game_root().or_else(|| kit.settings.with(|s| s.wow_path.clone())).unwrap_or_else(|| "Not found".into())}
            </p>
            <p class="lbl note dim">
                {move || match (chosen(), has_game()) {
                    (true, true) => "Chosen by hand.",
                    (true, false) => "Chosen by hand, but it no longer opens.",
                    (false, true) => "Found in the usual place.",
                    (false, false) => "Not in the usual places.",
                }}
            </p>
            <div class="set-actions">
                <button type="button" class="btn lbl" disabled=move || kit.busy.get() on:click=move |_| kit.choose_folder()>"Choose folder"</button>
                {move || chosen().then(|| view! {
                    <button type="button" class="btn lbl" disabled=move || kit.busy.get() on:click=move |_| kit.auto_folder()>"Find it again"</button>
                })}
                <button type="button" class="btn lbl" disabled=move || !has_game() on:click=move |_| kit.open("addons")>"Open AddOns"</button>
            </div>
        </section>
        <section class="set">
            <SetHead
                title="Backups"
                id="tip-backups"
                tip="Before an update or a removal changes a folder in AddOns, the old copy moves here, and so does Config.wtf before a graphics change. The last copy of each is kept."
            />
            <p class="body selectable path-line">"Interface/.gryphon-backup"</p>
            <div class="set-actions">
                <button type="button" class="btn lbl" disabled=move || !has_game() on:click=move |_| kit.open("backup")>"Open backups"</button>
            </div>
        </section>
        <section class="set">
            <SetHead
                title="GitHub token"
                id="tip-token"
                tip="Without one, GitHub allows 60 checks an hour from your network, plenty for a few addons. A token with no scopes raises that to 5,000. It's saved in plain text in the app's settings."
            />
            {move || {
                if kit.settings.with(|s| s.has_token) {
                    view! {
                        <div class="set-actions">
                            <span class="body">"Token saved"</span>
                            <button type="button" class="btn lbl" on:click=move |_| kit.set_token(String::new())>"Remove"</button>
                        </div>
                    }
                    .into_any()
                } else {
                    view! {
                        <form class="add-row" on:submit=save>
                            <input
                                class="field body"
                                type="password"
                                autocomplete="off"
                                placeholder="Optional: github_pat_..."
                                aria-label="GitHub token, optional"
                                prop:value=move || token.get()
                                on:input=move |ev| token.set(event_target_value(&ev))
                            />
                            <button type="submit" class="btn lbl" disabled=move || token.with(|t| t.trim().is_empty())>"Save"</button>
                        </form>
                    }
                    .into_any()
                }
            }}
        </section>
        <section class="set">
            <SetHead
                title="CurseForge key"
                id="tip-curseforge"
                tip="Lets Gryphon search and install from CurseForge. CurseForge only answers apps that send a key, and each player brings their own: it's free at console.curseforge.com, under API keys. It's saved in plain text in the app's settings."
            />
            {move || {
                if kit.settings.with(|s| s.has_curseforge_key) {
                    view! {
                        <div class="set-actions">
                            <span class="body">"Key saved"</span>
                            <button type="button" class="btn lbl" on:click=move |_| kit.set_curseforge_key(String::new())>"Remove"</button>
                        </div>
                    }
                    .into_any()
                } else {
                    view! { <CurseKey /> }.into_any()
                }
            }}
        </section>
    }
}
