//! GitHub Releases, where most addons come from. Public repos need no key; a token only raises
//! the hourly limit (60 requests an hour without one, one request per addon per check). The
//! client hands CurseForge projects to [`crate::curseforge`] when a key for it is set.

use std::path::Path;

use gryphon_shared::Source;
use reqwest::{StatusCode, header};
use serde::Deserialize;
use tokio::io::AsyncWriteExt;

use crate::curseforge::Curse;
use crate::error::{Result, msg};
use crate::wow::FOREVER_INTERFACES;

const API: &str = "https://api.github.com";
/// Pages of 100 releases to read looking for a tag prefix. Spoken's repo has over 100.
const MAX_PAGES: u32 = 4;
const MAX_RELEASES: usize = 10;

#[derive(Debug, Clone, Deserialize)]
pub struct Release {
    pub tag_name: String,
    #[serde(default)]
    pub prerelease: bool,
    #[serde(default)]
    pub draft: bool,
    pub published_at: Option<String>,
    pub html_url: String,
    #[serde(default)]
    pub assets: Vec<Asset>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Asset {
    pub name: String,
    pub size: u64,
    pub browser_download_url: String,
}

impl Asset {
    fn is_zip(&self) -> bool {
        self.name.to_ascii_lowercase().ends_with(".zip")
    }
}

/// The newest release of a source that fits Forever, and the zip to download.
#[derive(Debug, Clone)]
pub struct Latest {
    pub tag: String,
    pub published_at: Option<String>,
    pub html_url: String,
    pub asset: Asset,
}

/// `release.json`, which the BigWigs packager attaches to a release: each zip and the
/// clients it was built for.
#[derive(Debug, Deserialize)]
struct Manifest {
    releases: Vec<ManifestFile>,
}

#[derive(Debug, Deserialize)]
struct ManifestFile {
    filename: String,
    #[serde(default)]
    nolib: bool,
    #[serde(default)]
    metadata: Vec<ManifestTarget>,
}

#[derive(Debug, Deserialize)]
struct ManifestTarget {
    #[serde(default)]
    flavor: String,
    #[serde(default)]
    interface: u32,
}

impl ManifestFile {
    fn fits_forever(&self) -> bool {
        self.metadata.iter().any(|m| m.flavor.eq_ignore_ascii_case("forever") || FOREVER_INTERFACES.contains(&m.interface))
    }
}

/// Words in a tag or file name that mark a build for another client.
const OTHER_CLIENTS: &[&str] = &["retail", "classic", "vanilla", "era", "tbc", "bcc", "wrath", "wotlk", "cata", "mists", "mop", "anniversary"];
const FOREVER_WORDS: &[&str] = &["forever", "camelot"];

fn words(name: &str) -> Vec<String> {
    name.to_ascii_lowercase().split(|c: char| !c.is_ascii_alphanumeric()).filter(|w| !w.is_empty()).map(String::from).collect()
}

fn names_forever(name: &str) -> bool {
    words(name).iter().any(|w| FOREVER_WORDS.contains(&w.as_str()))
}

/// Says it is for another client, and not also for Forever.
fn names_other_client(name: &str) -> bool {
    let words = words(name);
    !words.iter().any(|w| FOREVER_WORDS.contains(&w.as_str())) && words.iter().any(|w| OTHER_CLIENTS.contains(&w.as_str()))
}

#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    token: Option<String>,
    curse: Option<Curse>,
}

impl Client {
    pub fn new(token: Option<String>) -> Result<Client> {
        let http = reqwest::Client::builder()
            .user_agent(concat!("Gryphon/", env!("CARGO_PKG_VERSION"), " (WoW addon manager)"))
            .build()?;
        Ok(Client { http, token: token.filter(|t| !t.trim().is_empty()), curse: None })
    }

    /// Adds a CurseForge API key, so CurseForge can be searched and its projects looked up.
    pub fn with_curseforge(mut self, key: Option<String>) -> Client {
        self.curse = key.map(|k| k.trim().to_string()).filter(|k| !k.is_empty()).map(|k| Curse::new(self.http.clone(), k));
        self
    }

    /// CurseForge's API, or what to do when there is no key for it.
    pub fn curseforge(&self) -> Result<&Curse> {
        self.curse.as_ref().ok_or_else(|| msg("CurseForge needs your own API key, free at console.curseforge.com. Add it in Settings."))
    }

    fn api(&self, url: &str) -> reqwest::RequestBuilder {
        let request = self.http.get(url).header(header::ACCEPT, "application/vnd.github+json").header("X-GitHub-Api-Version", "2022-11-28");
        match &self.token {
            Some(token) => request.bearer_auth(token),
            None => request,
        }
    }

    async fn releases_page(&self, repo: &str, page: u32) -> Result<Vec<Release>> {
        let response = self.api(&format!("{API}/repos/{repo}/releases?per_page=100&page={page}")).send().await?;
        match response.status() {
            StatusCode::NOT_FOUND => Err(msg(format!("GitHub has no repo {repo}. It may have moved or been renamed."))),
            StatusCode::FORBIDDEN | StatusCode::TOO_MANY_REQUESTS if remaining(&response) == Some(0) => {
                let minutes = reset_minutes(&response).map(|m| format!(" Try again in {m} min,")).unwrap_or_default();
                Err(msg(format!("GitHub's hourly limit for this network is used up.{minutes} or add a GitHub token in Settings.")))
            }
            status if !status.is_success() => Err(msg(format!("GitHub answered {status} for {repo}."))),
            _ => Ok(response.json().await?),
        }
    }

    /// The newest release of `source` built for Forever, and its zip. Looks back at most
    /// `MAX_RELEASES` releases: an addon with no Forever build in its last few has none. A
    /// CurseForge project's newest Forever file comes from CurseForge.
    pub async fn latest(&self, source: &Source) -> Result<Latest> {
        if let Some(project) = &source.curseforge {
            return self.curseforge()?.latest(project).await;
        }
        let mut looked = 0;
        for page in 1..=MAX_PAGES {
            let releases = self.releases_page(&source.repo, page).await?;
            let last_page = releases.len() < 100;
            for release in releases.iter().filter(|r| wanted(r, source)) {
                looked += 1;
                if looked > MAX_RELEASES {
                    break;
                }
                if let Some(asset) = self.pick_asset(release, source).await? {
                    return Ok(Latest {
                        tag: release.tag_name.clone(),
                        published_at: release.published_at.clone(),
                        html_url: release.html_url.clone(),
                        asset,
                    });
                }
            }
            if last_page || looked > MAX_RELEASES {
                break;
            }
        }
        let which = source.tag_prefix.as_deref().map(|p| format!(" tagged {p}…")).unwrap_or_default();
        Err(msg(format!("{} has no release{which} with a WoW Forever zip.", source.repo)))
    }

    /// The zip in a release that is the Forever build. `release.json` decides when it is
    /// there; otherwise the names do. The download is checked again after unpacking.
    async fn pick_asset(&self, release: &Release, source: &Source) -> Result<Option<Asset>> {
        let zips: Vec<&Asset> = release.assets.iter().filter(|a| a.is_zip()).collect();
        if let Some(want) = &source.asset {
            return Ok(zips.into_iter().find(|a| a.name.contains(want.as_str())).cloned());
        }
        if let Some(manifest) = release.assets.iter().find(|a| a.name == "release.json") {
            if let Ok(manifest) = self.manifest(manifest).await {
                let mut files: Vec<&ManifestFile> = manifest.releases.iter().filter(|f| f.fits_forever()).collect();
                files.sort_by_key(|f| f.nolib);
                return Ok(files.iter().find_map(|f| zips.iter().find(|a| a.name == f.filename)).map(|a| (*a).clone()));
            }
        }
        let full: Vec<&Asset> = zips.iter().copied().filter(|a| !a.name.to_ascii_lowercase().contains("nolib")).collect();
        let pool = if full.is_empty() { zips } else { full };
        if let Some(named) = pool.iter().find(|a| names_forever(&a.name)) {
            return Ok(Some((*named).clone()));
        }
        Ok(pool.into_iter().filter(|a| !names_other_client(&a.name)).min_by_key(|a| a.name.len()).cloned())
    }

    async fn manifest(&self, asset: &Asset) -> Result<Manifest> {
        Ok(self.http.get(&asset.browser_download_url).send().await?.error_for_status()?.json().await?)
    }

    /// Streams `url` into `dest`, reporting bytes as they arrive. No GitHub token is sent:
    /// release files are public, and the download is served from another host. CurseForge's
    /// file servers get the CurseForge key, which they ask for.
    pub async fn download(&self, url: &str, dest: &Path, mut on_bytes: impl FnMut(u64, Option<u64>)) -> Result<()> {
        let mut request = self.http.get(url).header(header::ACCEPT, "application/octet-stream");
        if let Some(key) = self.curse.as_ref().and_then(|c| c.key_for(url)) {
            request = request.header("x-api-key", key);
        }
        let mut response = request.send().await?.error_for_status()?;
        let total = response.content_length();
        let mut file = tokio::fs::File::create(dest).await?;
        let mut done = 0u64;
        on_bytes(0, total);
        while let Some(chunk) = response.chunk().await? {
            file.write_all(&chunk).await?;
            done += chunk.len() as u64;
            on_bytes(done, total);
        }
        file.flush().await?;
        Ok(())
    }
}

fn wanted(release: &Release, source: &Source) -> bool {
    !release.draft
        && !release.prerelease
        && source.tag_prefix.as_deref().is_none_or(|p| release.tag_name.starts_with(p))
        && !names_other_client(&release.tag_name)
}

fn remaining(response: &reqwest::Response) -> Option<u64> {
    response.headers().get("x-ratelimit-remaining")?.to_str().ok()?.parse().ok()
}

fn reset_minutes(response: &reqwest::Response) -> Option<u64> {
    let reset: u64 = response.headers().get("x-ratelimit-reset")?.to_str().ok()?.parse().ok()?;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();
    Some(reset.saturating_sub(now).div_ceil(60).max(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_client_words() {
        assert!(names_other_client("v0.7.1-beta-retail"));
        assert!(names_other_client("SpokenQuests-WoW_1.12-classic.zip"));
        assert!(!names_other_client("v0.7.5-beta"));
        assert!(!names_other_client("bundle/v12.3.1+v1.0.9"));
        assert!(!names_other_client("Addon-classic-forever.zip"));
        assert!(names_forever("OpenMap-v0.4.0-beta-forever.zip"));
    }

    #[test]
    fn filters_releases() {
        let release = |tag: &str, pre: bool| Release {
            tag_name: tag.into(),
            prerelease: pre,
            draft: false,
            published_at: None,
            html_url: String::new(),
            assets: vec![],
        };
        let spoken = Source { tag_prefix: Some("spoken/v".into()), ..Source::github("rusty-key/spoken-wow") };
        assert!(wanted(&release("spoken/v3.6.0", false), &spoken));
        assert!(!wanted(&release("spoken/v3.6.0-alpha.2", true), &spoken));
        assert!(!wanted(&release("quests-audio/v2.4.0", false), &spoken));
        assert!(!wanted(&release("v0.7.1-beta-retail", false), &Source::github("openmap-world/openmap")));
    }
}
