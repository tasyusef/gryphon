//! `gryphon`: install and update WoW Forever addons from a terminal.

use std::path::PathBuf;
use std::process::ExitCode;

use gryphon_core::github::Client;
use gryphon_core::shared::{Source, Step};
use gryphon_core::state::State;
use gryphon_core::wow::Game;
use gryphon_core::graphics::{self, Change};
use gryphon_core::{catalog, install, library};

const HELP: &str = "gryphon: WoW Forever addons from GitHub

usage: gryphon [--wow <folder>] <command>

commands:
  status               the game, and what is installed
  list                 the built-in picks
  check                look for updates
  install <addon>...   a pick's id (openmap) or a GitHub repo (owner/name or URL)
  update               update everything that has an update
  remove <id>...       take addons out (their folders go to the backup)
  pack [name]          print what is installed as a list to share
  graphics             the game settings Gryphon can change, and where they stand
  graphics <setting> <value|back>
                       change one (the game must be closed), or put it back

--wow takes the World of Warcraft folder or anything inside it. GITHUB_TOKEN is used when set.";

#[tokio::main]
async fn main() -> ExitCode {
    match run(std::env::args().skip(1).collect()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

async fn run(mut args: Vec<String>) -> Result<(), String> {
    let mut wow = None;
    if let Some(i) = args.iter().position(|a| a == "--wow") {
        args.remove(i);
        wow = Some(PathBuf::from(if i < args.len() { args.remove(i) } else { return Err("--wow needs a folder".into()) }));
    }
    let Some(command) = args.first().cloned() else {
        println!("{HELP}");
        return Ok(());
    };
    let rest = &args[1..];
    if command == "list" {
        for entry in catalog::CATALOG {
            println!("{:<22} {:<22} {}", entry.id, entry.name, entry.blurb);
        }
        return Ok(());
    }
    if matches!(command.as_str(), "help" | "-h" | "--help") {
        println!("{HELP}");
        return Ok(());
    }

    let game = match &wow {
        Some(path) => Game::open(path).ok_or_else(|| format!("no WoW Forever install at {}", path.display()))?,
        None => Game::find().ok_or("WoW Forever was not found in the usual places; pass --wow <folder>")?,
    };
    let client = Client::new(std::env::var("GITHUB_TOKEN").ok()).map_err(|e| e.to_string())?;

    match command.as_str() {
        "status" => status(&game),
        "check" => check(&client, &game).await,
        "install" => {
            if rest.is_empty() {
                return Err("install what? try `gryphon list`".into());
            }
            for want in rest {
                let (id, name, source) = resolve(want)?;
                for need in catalog::find(&id).map(|e| e.needs).unwrap_or_default() {
                    if !State::load(&game).map_err(|e| e.to_string())?.addons.contains_key(*need) {
                        let entry = catalog::find(need).expect("catalog needs name catalog ids");
                        install_one(&client, &game, entry.id, entry.name, &entry.source()).await?;
                    }
                }
                install_one(&client, &game, &id, &name, &source).await?;
            }
            Ok(())
        }
        "update" => {
            let state = State::load(&game).map_err(|e| e.to_string())?;
            let sources = state.addons.iter().map(|(id, a)| (id.clone(), a.source.clone())).collect();
            let mut updated = 0;
            for (id, result) in library::check(&client, sources).await {
                let record = &state.addons[&id];
                match result {
                    Ok(latest) if latest.tag != record.tag => {
                        let mut on_step = progress(&record.name);
                        install::install_release(&client, &game, &id, &record.name, &record.source, &latest, &mut on_step)
                            .await
                            .map_err(|e| e.to_string())?;
                        updated += 1;
                    }
                    Ok(_) => {}
                    Err(e) => eprintln!("{}: {e}", record.name),
                }
            }
            println!("{updated} updated");
            Ok(())
        }
        "remove" => {
            for id in rest {
                let removed = install::remove(&game, id).map_err(|e| e.to_string())?;
                println!("removed {} ({})", removed.name, removed.folders.join(", "));
            }
            Ok(())
        }
        "pack" => {
            let name = rest.first().map(String::as_str).unwrap_or("My addons");
            let pack = library::pack(&game, name).map_err(|e| e.to_string())?;
            println!("{}", serde_json::to_string_pretty(&pack).map_err(|e| e.to_string())?);
            Ok(())
        }
        "graphics" => match rest {
            [] => show_graphics(&game),
            [id, value] => {
                let change = match value.as_str() {
                    "back" => Change::PutBack,
                    number => Change::Set(number.parse().map_err(|_| format!("{number} is not a whole number"))?),
                };
                graphics::change(&game, &[(id.clone(), change)]).map_err(|e| e.to_string())?;
                show_graphics(&game)
            }
            _ => Err("usage: gryphon graphics <setting> <value|back>".into()),
        },
        other => Err(format!("unknown command {other}; try `gryphon help`")),
    }
}

/// A pick's id or repo, or any GitHub repo.
fn resolve(want: &str) -> Result<(String, String, Source), String> {
    let source = Source::parse(want);
    if let Some(entry) = catalog::find(want).or_else(|| source.as_ref().and_then(catalog::for_source)) {
        return Ok((entry.id.to_string(), entry.name.to_string(), entry.source()));
    }
    let source = source.ok_or_else(|| format!("{want} is not a pick or a GitHub repo"))?;
    let name = source.repo.rsplit('/').next().unwrap_or(&source.repo).to_string();
    Ok((library::id_for(&source), name, source))
}

async fn install_one(client: &Client, game: &Game, id: &str, name: &str, source: &Source) -> Result<(), String> {
    let mut on_step = progress(name);
    let installed = install::install(client, game, id, name, source, &mut on_step).await.map_err(|e| e.to_string())?;
    println!("{name} {} installed: {}", installed.tag, installed.folders.join(", "));
    Ok(())
}

fn progress(name: &str) -> impl FnMut(Step) + Send + use<> {
    let name = name.to_string();
    move |step| match step {
        Step::Checking => eprintln!("{name}: checking GitHub"),
        Step::Downloading { done, total: Some(total) } if done == total || done == 0 => {
            eprintln!("{name}: downloading {} / {} MB", done >> 20, total >> 20)
        }
        Step::Unpacking => eprintln!("{name}: unpacking and checking for a Forever build"),
        _ => {}
    }
}

fn status(game: &Game) -> Result<(), String> {
    println!("game     {}", game.root.display());
    println!("build    {} (interface {})", game.version.as_deref().unwrap_or("unknown"), game.interface.map_or("?".into(), |i| i.to_string()));
    println!("addons   {}", game.addons_dir().display());
    let library = library::library(Some(game), &library::Checks::new()).map_err(|e| e.to_string())?;
    println!();
    for addon in library.addons.iter().filter(|a| a.installed.is_some()) {
        let installed = addon.installed.as_ref().unwrap();
        println!("{:<34} {:<22} {}", addon.id, installed.tag, installed.folders.join(", "));
    }
    if !library.other.is_empty() {
        println!("\nnot managed:");
        for folder in &library.other {
            println!("  {:<32} {:?} {:?}", folder.name, folder.fit, folder.interface);
        }
    }
    Ok(())
}

fn show_graphics(game: &Game) -> Result<(), String> {
    let graphics = graphics::read(game).map_err(|e| e.to_string())?;
    if graphics.missing {
        println!("the game has not saved its settings yet; start it once");
    } else if graphics.running {
        println!("the game is open; close it to change these");
    }
    for tweak in &graphics.tweaks {
        let before = tweak.before.map_or(String::new(), |b| format!("was {b}"));
        println!("{:<22} {:<18} {:>6}  {:>5}-{:<6} {before}", tweak.id, tweak.name, tweak.value, tweak.min, tweak.max);
    }
    Ok(())
}

async fn check(client: &Client, game: &Game) -> Result<(), String> {
    let state = State::load(game).map_err(|e| e.to_string())?;
    let mut sources: Vec<(String, Source)> = catalog::CATALOG.iter().map(|e| (e.id.to_string(), e.source())).collect();
    sources.extend(state.addons.iter().filter(|(id, _)| catalog::find(id).is_none()).map(|(id, a)| (id.clone(), a.source.clone())));
    for (id, result) in library::check(client, sources).await {
        let have = state.addons.get(&id).map(|a| a.tag.as_str());
        match result {
            Ok(latest) => {
                let mark = match have {
                    Some(tag) if tag == latest.tag => "up to date",
                    Some(_) => "UPDATE",
                    None => "not installed",
                };
                println!("{id:<34} {:<26} {:>6} MB  {:<14} {}", latest.tag, latest.asset.size >> 20, mark, latest.asset.name);
            }
            Err(e) => println!("{id:<34} {e}"),
        }
    }
    Ok(())
}
