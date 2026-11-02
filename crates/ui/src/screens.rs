//! The screens: the picks, addons added from GitHub, the other folders in AddOns, and
//! settings. Each is a page head (a label, the title, one line, and where the game is) over its
//! rows.

use gryphon_shared::{Addon, Fit, Folder, Step};
use leptos::prelude::*;

use crate::app::Kit;
use crate::text;

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
        let added: Vec<Addon> = kit.library.with(|l| l.as_ref().map(|l| l.addons.iter().filter(|a| !a.featured).cloned().collect()).unwrap_or_default());
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
    let version = |tag: &str| text::version(tag, &addon.source);
    let (now, then) = match (&addon.installed, &addon.latest) {
        (Some(have), Some(latest)) if update => (format!("v{}", version(&have.tag)), Some(format!("New v{}", version(&latest.tag)))),
        (Some(have), _) => (format!("v{}", version(&have.tag)), None),
        (None, Some(latest)) => (format!("v{}", version(&latest.tag)), None),
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
    let blurb = addon.blurb.clone().unwrap_or_else(|| addon.source.repo.clone());
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
    let back = if addon.featured { "Picks" } else { "From GitHub" };
    let Standing { now, then, .. } = standing(&addon);
    let size = size_of(&addon).map(text::size);
    let released = addon.latest.as_ref().and_then(|l| l.published_at.as_deref()).map(text::date);
    let folders = addon.installed.as_ref().map(|i| i.folders.join(", ")).filter(|f| !f.is_empty());
    let needs = needs_names(kit, &addon, false);
    let problem = addon.problem.clone();
    let blurb = addon.blurb.clone().unwrap_or_else(|| addon.source.repo.clone());
    let repo = addon.source.repo.clone();
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
                    <button type="button" class="btn lbl" on:click={
                        let repo = repo.clone();
                        move |_| kit.open_github(repo.clone())
                    }>"Open on GitHub"</button>
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
                <dd class="lbl note selectable">{repo}</dd>
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
fn SetHead(title: &'static str, id: &'static str, tip: &'static str) -> impl IntoView {
    view! {
        <div class="set-head">
            <h2 class="lbl dim">{title}</h2>
            <span class="tip">
                <button type="button" class="tip-mark lbl" aria-label=format!("About {}", title.to_lowercase()) aria-describedby=id>
                    "?"
                </button>
                <span class="tip-body lbl note" role="tooltip" id=id>{tip}</span>
            </span>
        </div>
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
                tip="Before an update or a removal changes a folder in AddOns, the old copy moves here. The last copy of each folder is kept."
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
    }
}
