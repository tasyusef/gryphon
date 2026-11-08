//! The library: the catalog, what is installed and what else sits in AddOns, as one list.

use std::collections::BTreeMap;

use gryphon_shared::{Addon, InstalledInfo, LatestInfo, Library, Pack, PackAddon, Search, Source};

use crate::catalog::{self, CATALOG};
use crate::error::Result;
use crate::github::{Client, Latest};
use crate::state::State;
use crate::toc;
use crate::wow::Game;

/// The last check of each addon: its newest Forever release, or why it could not be found.
pub type Checks = BTreeMap<String, std::result::Result<LatestInfo, String>>;

/// The id an addon added from GitHub gets: its repo, lowercased, plus any tag prefix. One from
/// CurseForge gets its project number, which stays put when the project is renamed.
pub fn id_for(source: &Source) -> String {
    if let Some(project) = &source.curseforge {
        return format!("curseforge:{}", project.id);
    }
    let repo = source.repo.to_ascii_lowercase();
    match &source.tag_prefix {
        Some(prefix) => format!("{repo}#{}", prefix.trim_end_matches(['/', 'v'])),
        None => repo,
    }
}

pub fn library(game: Option<&Game>, checks: &Checks) -> Result<Library> {
    let state = match game {
        Some(game) => State::load(game)?,
        None => State::default(),
    };
    let check = |id: &str| match checks.get(id) {
        Some(Ok(latest)) => (Some(latest.clone()), None),
        Some(Err(problem)) => (None, Some(problem.clone())),
        None => (None, None),
    };
    let installed = |id: &str| {
        state.addons.get(id).map(|a| InstalledInfo { tag: a.tag.clone(), folders: a.folders.clone(), installed_at: a.installed_at })
    };

    let mut addons: Vec<Addon> = CATALOG
        .iter()
        .map(|entry| {
            let (latest, problem) = check(entry.id);
            Addon {
                id: entry.id.to_string(),
                name: entry.name.to_string(),
                blurb: Some(entry.blurb.to_string()),
                about: entry.about.iter().map(|p| p.to_string()).collect(),
                source: entry.source(),
                featured: true,
                needs: entry.needs.iter().map(|n| n.to_string()).collect(),
                size_hint_mb: Some(entry.size_hint_mb),
                installed: installed(entry.id),
                latest,
                problem,
            }
        })
        .collect();
    for (id, record) in state.addons.iter().filter(|(id, _)| catalog::find(id).is_none()) {
        let (latest, problem) = check(id);
        addons.push(Addon {
            id: id.clone(),
            name: record.name.clone(),
            blurb: None,
            about: vec![],
            source: record.source.clone(),
            featured: false,
            needs: vec![],
            size_hint_mb: None,
            installed: installed(id),
            latest,
            problem,
        });
    }

    let other = match game {
        Some(game) => toc::scan(&game.addons_dir()).into_iter().filter(|f| state.owner_of(&f.name).is_none()).collect(),
        None => vec![],
    };
    Ok(Library { game: game.map(Game::info), addons, other })
}

/// Looks up the newest Forever release of each source, a few at a time.
pub async fn check(client: &Client, sources: Vec<(String, Source)>) -> BTreeMap<String, Result<Latest>> {
    let mut results = BTreeMap::new();
    let mut tasks = tokio::task::JoinSet::new();
    let mut queue = sources.into_iter();
    loop {
        while tasks.len() < 4 {
            let Some((id, source)) = queue.next() else { break };
            let client = client.clone();
            tasks.spawn(async move { (id, client.latest(&source).await) });
        }
        match tasks.join_next().await {
            Some(Ok((id, result))) => {
                results.insert(id, result);
            }
            Some(Err(_)) => {}
            None => break,
        }
    }
    results
}

/// Marks each search result already in the library: installed from CurseForge, or shipping a
/// folder an addon installed here already has, so installing it would replace that addon.
pub fn mark_installed(search: &mut Search, game: Option<&Game>) -> Result<()> {
    let Some(game) = game else { return Ok(()) };
    let state = State::load(game)?;
    let named = |id: &str| state.addons.get(id).map(|a| (id.to_string(), a.name.clone()));
    for found in &mut search.results {
        let id = id_for(&Source::curseforge(found.id, &found.slug));
        found.installed_as = named(&id).or_else(|| found.folders.iter().find_map(|f| state.owner_of(f)).and_then(named));
    }
    Ok(())
}

pub fn latest_info(latest: &Latest) -> LatestInfo {
    LatestInfo { tag: latest.tag.clone(), published_at: latest.published_at.clone(), size: latest.asset.size }
}

/// Everything installed, as a list a friend can import.
pub fn pack(game: &Game, name: &str) -> Result<Pack> {
    let state = State::load(game)?;
    Ok(Pack {
        name: name.to_string(),
        addons: state.addons.iter().map(|(id, a)| PackAddon { id: id.clone(), name: a.name.clone(), source: a.source.clone() }).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_for_github_addons() {
        assert_eq!(id_for(&Source::github("Questie/Questie")), "questie/questie");
        let spoken = Source { tag_prefix: Some("quests-audio/v".into()), ..Source::github("rusty-key/spoken-wow") };
        assert_eq!(id_for(&spoken), "rusty-key/spoken-wow#quests-audio");
        assert_eq!(id_for(&Source::curseforge(13592, "masque")), "curseforge:13592");
    }

    #[test]
    fn lists_the_catalog_without_a_game() {
        let library = library(None, &Checks::new()).unwrap();
        assert_eq!(library.addons.len(), CATALOG.len());
        assert!(library.addons.iter().all(|a| a.featured && a.installed.is_none()));
    }
}
