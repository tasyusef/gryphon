//! The library: the catalog, what is installed and what else sits in AddOns, as one list.

use std::collections::BTreeMap;

use gryphon_shared::{Addon, InstalledInfo, LatestInfo, Library, Pack, PackAddon, Source};

use crate::catalog::{self, CATALOG};
use crate::error::Result;
use crate::github::{Client, Latest};
use crate::state::State;
use crate::toc;
use crate::wow::Game;

/// The last check of each addon: its newest Forever release, or why it could not be found.
pub type Checks = BTreeMap<String, std::result::Result<LatestInfo, String>>;

/// The id an addon added from GitHub gets: its repo, lowercased, plus any tag prefix.
pub fn id_for(source: &Source) -> String {
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
        let spoken = Source { repo: "rusty-key/spoken-wow".into(), tag_prefix: Some("quests-audio/v".into()), asset: None };
        assert_eq!(id_for(&spoken), "rusty-key/spoken-wow#quests-audio");
    }

    #[test]
    fn lists_the_catalog_without_a_game() {
        let library = library(None, &Checks::new()).unwrap();
        assert_eq!(library.addons.len(), CATALOG.len());
        assert!(library.addons.iter().all(|a| a.featured && a.installed.is_none()));
    }
}
