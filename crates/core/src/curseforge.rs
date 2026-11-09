//! CurseForge: searching it, and finding each project's newest WoW Forever file. Its API needs a
//! key, and each player brings their own (free at console.curseforge.com), since an app's key
//! can't be shared. Since 16 July 2026 CurseForge's file servers want the key too
//! (blog.curseforge.com, "API key authentication for CurseForge file downloads").

use gryphon_shared::{CurseProject, Found, Search, curseforge_url};
use reqwest::{StatusCode, Url, header};
use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::error::{Result, msg};
use crate::github::{Asset, Latest};

const API: &str = "https://api.curseforge.com/v1";
/// World of Warcraft, in CurseForge's list of games.
const WOW: &str = "1";
/// The game version type CurseForge files WoW Forever builds under (`/v1/games/1/version-types`).
pub const FOREVER_TYPE: u64 = 88568;
/// Search results per page.
const PAGE_SIZE: u32 = 30;
/// Sort by popularity (CurseForge's `ModsSearchSortField`).
const BY_POPULARITY: &str = "2";
/// A full release (`FileReleaseType`), rather than a beta or an alpha.
const RELEASE: u8 = 1;

#[derive(Deserialize)]
struct Data<T> {
    data: T,
}

#[derive(Deserialize)]
struct Page<T> {
    data: T,
    #[serde(default)]
    pagination: Option<Pagination>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Pagination {
    #[serde(default)]
    total_count: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Mod {
    id: u64,
    name: String,
    slug: String,
    #[serde(default)]
    summary: String,
    /// A whole number, but read as a float so a value past what JSON integers hold still reads.
    #[serde(default)]
    download_count: f64,
    #[serde(default)]
    authors: Vec<Author>,
    #[serde(default)]
    latest_files: Vec<File>,
    #[serde(default)]
    date_released: Option<String>,
    /// `false` when the author keeps their files to CurseForge's own app.
    #[serde(default)]
    allow_mod_distribution: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct Author {
    name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct File {
    id: u64,
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    file_name: String,
    #[serde(default)]
    release_type: u8,
    #[serde(default)]
    file_date: Option<String>,
    #[serde(default)]
    file_length: u64,
    /// `None` when the author doesn't allow downloads outside CurseForge's app.
    #[serde(default)]
    download_url: Option<String>,
    #[serde(default = "yes")]
    is_available: bool,
    #[serde(default)]
    expose_as_alternative: Option<bool>,
    #[serde(default)]
    sortable_game_versions: Vec<GameVersion>,
    #[serde(default)]
    modules: Vec<Module>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GameVersion {
    #[serde(default)]
    game_version_type_id: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct Module {
    name: String,
}

fn yes() -> bool {
    true
}

impl File {
    fn fits_forever(&self) -> bool {
        self.is_available
            && self.expose_as_alternative != Some(true)
            && self.sortable_game_versions.iter().any(|v| v.game_version_type_id == Some(FOREVER_TYPE))
    }

    /// The name it goes by: `11.2.10`, or the zip's name when the author gave none.
    fn shown_name(&self) -> &str {
        if self.display_name.trim().is_empty() { &self.file_name } else { self.display_name.trim() }
    }
}

/// The newest Forever file: the newest full release, or the newest beta or alpha when there is
/// no full release. File ids only grow, so the biggest is the newest.
fn newest(files: &[File]) -> Option<&File> {
    let forever: Vec<&File> = files.iter().filter(|f| f.fits_forever()).collect();
    let release = forever.iter().copied().filter(|f| f.release_type == RELEASE).max_by_key(|f| f.id);
    release.or_else(|| forever.into_iter().max_by_key(|f| f.id))
}

/// A file's tag, as Gryphon records it: the name the author gave it, then its id, so a file
/// uploaded again under the same name still counts as new. `11.2.10#7398127`.
pub fn tag(display: &str, file_id: u64) -> String {
    format!("{display}#{file_id}")
}

impl Mod {
    fn found(self) -> Found {
        let file = newest(&self.latest_files);
        let folders = file.map(|f| f.modules.iter().map(|m| m.name.clone()).collect()).unwrap_or_default();
        let updated = file.and_then(|f| f.file_date.clone()).or(self.date_released.clone());
        Found {
            id: self.id,
            forever: file.is_some(),
            distributable: self.allow_mod_distribution != Some(false),
            folders,
            updated,
            slug: self.slug,
            name: self.name,
            summary: self.summary,
            authors: self.authors.into_iter().map(|a| a.name).collect(),
            downloads: self.download_count.max(0.0) as u64,
            installed_as: None,
        }
    }
}

/// CurseForge's API, with the player's key.
#[derive(Clone)]
pub struct Curse {
    http: reqwest::Client,
    key: String,
}

impl Curse {
    pub fn new(http: reqwest::Client, key: String) -> Curse {
        Curse { http, key }
    }

    /// The key, for a download from CurseForge's own file servers (edge.forgecdn.net, which
    /// redirects to mediafilez.forgecdn.net) and nowhere else.
    pub fn key_for(&self, url: &str) -> Option<&str> {
        let url = Url::parse(url).ok()?;
        let host = url.host_str()?;
        (url.scheme() == "https" && (host == "forgecdn.net" || host.ends_with(".forgecdn.net"))).then_some(self.key.as_str())
    }

    async fn get<T: DeserializeOwned>(&self, url: Url, missing: impl FnOnce() -> String) -> Result<T> {
        let response = self.http.get(url).header("x-api-key", &self.key).header(header::ACCEPT, "application/json").send().await?;
        match response.status() {
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                // A key pasted right gets this too until CurseForge approves it (2026-11).
                Err(msg("CurseForge turned down the API key. If it's pasted right, CurseForge may not have approved it yet. Check console.curseforge.com."))
            }
            StatusCode::NOT_FOUND => Err(msg(missing())),
            StatusCode::TOO_MANY_REQUESTS => Err(msg("CurseForge says that's too many requests for now. Try again in a minute.")),
            status if !status.is_success() => Err(msg(format!("CurseForge answered {status}."))),
            _ => Ok(response.json().await?),
        }
    }

    /// Addons with a WoW Forever file, most popular first. No words lists the most popular.
    pub async fn search(&self, words: &str) -> Result<Search> {
        let mut params = vec![
            ("gameId", WOW.to_string()),
            ("gameVersionTypeId", FOREVER_TYPE.to_string()),
            ("sortField", BY_POPULARITY.to_string()),
            ("sortOrder", "desc".to_string()),
            ("pageSize", PAGE_SIZE.to_string()),
            ("index", "0".to_string()),
        ];
        let words = words.trim();
        if !words.is_empty() {
            params.push(("searchFilter", words.to_string()));
        }
        let url = Url::parse_with_params(&format!("{API}/mods/search"), &params).map_err(|e| msg(e.to_string()))?;
        let page: Page<Vec<Mod>> = self.get(url, || "CurseForge's search isn't answering.".to_string()).await?;
        Ok(search_page(page))
    }

    /// One project, as a search would show it.
    pub async fn project(&self, id: u64) -> Result<Found> {
        Ok(self.project_raw(id).await?.found())
    }

    async fn project_raw(&self, id: u64) -> Result<Mod> {
        let url = Url::parse(&format!("{API}/mods/{id}")).map_err(|e| msg(e.to_string()))?;
        let data: Data<Mod> = self.get(url, || format!("CurseForge has no project {id}. It may have been taken down.")).await?;
        Ok(data.data)
    }

    /// The newest WoW Forever file of a project, and where to download it.
    pub async fn latest(&self, project: &CurseProject) -> Result<Latest> {
        let found = self.project_raw(project.id).await?;
        if let Some(file) = newest(&found.latest_files) {
            return to_latest(&found, file);
        }
        // the newest files cover each client; an older Forever file can sit further back
        let url = Url::parse_with_params(
            &format!("{API}/mods/{}/files", project.id),
            &[("gameVersionTypeId", FOREVER_TYPE.to_string()), ("pageSize", "50".to_string())],
        )
        .map_err(|e| msg(e.to_string()))?;
        let files: Page<Vec<File>> = self.get(url, || format!("CurseForge has no files for {}.", found.name)).await?;
        match newest(&files.data) {
            Some(file) => to_latest(&found, file),
            None => Err(msg(format!("{} has no file built for WoW Forever on CurseForge.", found.name))),
        }
    }
}

fn search_page(page: Page<Vec<Mod>>) -> Search {
    // the search is asked for Forever files only; this keeps that true if it ever isn't
    let results = page.data.into_iter().map(Mod::found).filter(|f| f.forever).collect();
    Search { results, total: page.pagination.and_then(|p| p.total_count) }
}

fn to_latest(project: &Mod, file: &File) -> Result<Latest> {
    let Some(url) = file.download_url.clone().filter(|u| !u.is_empty()) else {
        return Err(msg(if project.allow_mod_distribution == Some(false) {
            format!("{}'s author only lets CurseForge's own app download it. Get it there, or from its page on curseforge.com.", project.name)
        } else {
            format!("CurseForge has no download for {} {} right now. Try again later.", project.name, file.shown_name())
        }));
    };
    Ok(Latest {
        tag: tag(file.shown_name(), file.id),
        published_at: file.file_date.clone(),
        html_url: curseforge_url(&project.slug),
        asset: Asset { name: file.file_name.clone(), size: file.file_length, browser_download_url: url },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sends_the_key_only_to_curseforges_file_servers() {
        let curse = Curse::new(reqwest::Client::new(), "k".into());
        assert_eq!(curse.key_for("https://edge.forgecdn.net/files/1/2/a.zip"), Some("k"));
        assert_eq!(curse.key_for("https://mediafilez.forgecdn.net/files/1/2/a.zip"), Some("k"));
        assert_eq!(curse.key_for("http://edge.forgecdn.net/files/1/2/a.zip"), None);
        assert_eq!(curse.key_for("https://github.com/x/y/releases/download/v1/a.zip"), None);
        assert_eq!(curse.key_for("https://notforgecdn.net/a.zip"), None);
        assert_eq!(curse.key_for("https://forgecdn.net.example.com/a.zip"), None);
    }

    fn file(id: u64, release: u8, types: &[u64], url: Option<&str>) -> serde_json::Value {
        json!({
            "id": id, "displayName": format!("1.0.{id}"), "fileName": format!("Foo-1.0.{id}.zip"),
            "releaseType": release, "fileDate": "2026-11-02T18:00:00Z", "fileLength": 2048,
            "downloadUrl": url, "isAvailable": true,
            "sortableGameVersions": types.iter().map(|t| json!({ "gameVersionTypeId": t })).collect::<Vec<_>>(),
            "modules": [{ "name": "Foo", "fingerprint": 1 }, { "name": "Foo_Options", "fingerprint": 2 }],
        })
    }

    fn project(files: Vec<serde_json::Value>, allowed: bool) -> Mod {
        serde_json::from_value(json!({
            "id": 42, "name": "Foo", "slug": "foo", "summary": "Does foo.", "downloadCount": 1234.0,
            "authors": [{ "id": 1, "name": "Ann" }], "latestFiles": files,
            "dateReleased": "2026-11-01T00:00:00Z", "allowModDistribution": allowed,
        }))
        .unwrap()
    }

    #[test]
    fn picks_the_newest_forever_release() {
        let p = project(
            vec![
                file(10, RELEASE, &[FOREVER_TYPE], Some("https://edge.forgecdn.net/a.zip")),
                file(12, 2, &[FOREVER_TYPE], Some("https://edge.forgecdn.net/b.zip")),
                file(15, RELEASE, &[517], Some("https://edge.forgecdn.net/c.zip")),
            ],
            true,
        );
        assert_eq!(newest(&p.latest_files).map(|f| f.id), Some(10));
        let latest = to_latest(&p, newest(&p.latest_files).unwrap()).unwrap();
        assert_eq!(latest.tag, "1.0.10#10");
        assert_eq!(latest.asset.browser_download_url, "https://edge.forgecdn.net/a.zip");
        assert_eq!(latest.html_url, "https://www.curseforge.com/wow/addons/foo");
    }

    #[test]
    fn takes_a_beta_when_there_is_no_release() {
        let p = project(vec![file(12, 2, &[FOREVER_TYPE], Some("u")), file(14, 3, &[FOREVER_TYPE], Some("u"))], true);
        assert_eq!(newest(&p.latest_files).map(|f| f.id), Some(14));
    }

    #[test]
    fn says_when_the_author_keeps_it_to_curseforge() {
        let p = project(vec![file(10, RELEASE, &[FOREVER_TYPE], None)], false);
        let error = to_latest(&p, newest(&p.latest_files).unwrap()).unwrap_err().to_string();
        assert!(error.contains("only lets CurseForge's own app"), "{error}");
    }

    #[test]
    fn reads_a_search_page() {
        let page: Page<Vec<Mod>> = serde_json::from_value(json!({
            "data": [
                { "id": 42, "name": "Foo", "slug": "foo", "summary": "Does foo.", "downloadCount": 1234,
                  "authors": [{ "name": "Ann" }], "allowModDistribution": true,
                  "latestFiles": [file(10, RELEASE, &[FOREVER_TYPE], Some("u"))] },
                { "id": 43, "name": "Retail only", "slug": "retail-only", "latestFiles": [file(11, RELEASE, &[517], Some("u"))] },
            ],
            "pagination": { "index": 0, "pageSize": 30, "resultCount": 2, "totalCount": 2 },
        }))
        .unwrap();
        let search = search_page(page);
        assert_eq!(search.total, Some(2));
        assert_eq!(search.results.len(), 1);
        let foo = &search.results[0];
        assert_eq!((foo.id, foo.slug.as_str(), foo.downloads), (42, "foo", 1234));
        assert_eq!(foo.folders, ["Foo", "Foo_Options"]);
        assert_eq!(foo.authors, ["Ann"]);
        assert!(foo.forever && foo.distributable);
    }
}
