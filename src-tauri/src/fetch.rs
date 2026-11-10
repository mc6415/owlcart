//! In-app version of scripts/build-data.ts: looks up covers (SteamGridDB) and
//! game details (IGDB) for whole consoles and writes them to a local data
//! folder in the same layout as the published data (see data.rs).

use crate::covers::{Covers, MATCH_THRESHOLD, match_score, search_query};
use crate::data::title_bucket;
use crate::scrape::{self, EntryKind};
use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

const SGDB_CDN: &str = "https://cdn2.steamgriddb.com/";

/// A console to fetch, as listed in consoles.json.
#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleSpec {
    pub name: String,
    pub group: String,
    #[serde(default)]
    pub folder: Option<String>,
    pub listing: String,
    #[serde(default)]
    pub igdb: Option<String>,
}

#[derive(Serialize, Clone, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FetchStatus {
    pub running: bool,
    pub console: String,
    pub done: u64,
    pub total: u64,
    pub covers: u64,
    pub details: u64,
    pub titles: u64,
    /// Consoles still waiting after this one.
    pub queued: Vec<String>,
    /// Last error or a completion note.
    pub message: Option<String>,
}

#[derive(Default)]
pub struct Fetcher {
    pub status: Mutex<FetchStatus>,
    pub cancel: AtomicBool,
}

pub struct Keys {
    pub sgdb: String,
    pub igdb_id: String,
    pub igdb_secret: String,
}

/// Same as listingSlug() in src/lib/names.ts.
pub fn listing_slug(listing: &str) -> String {
    let path = url::Url::parse(listing)
        .map(|u| u.path().to_string())
        .unwrap_or_default();
    let decoded = percent_encoding::percent_decode_str(&path).decode_utf8_lossy();
    let rest = decoded
        .strip_prefix("/browse/")
        .unwrap_or(&decoded)
        .trim_start_matches("./");
    let mut slug = String::new();
    for c in rest.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c);
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    slug.trim_matches('-').to_string()
}

/// Everything known for one console, loaded from and saved to its folder.
struct ConsoleData {
    dir: PathBuf,
    covers: Map<String, Value>,
    details: BTreeMap<String, Value>,
}

impl ConsoleData {
    async fn load(dir: PathBuf) -> Self {
        let read = |p: PathBuf| async move {
            tokio::fs::read_to_string(p)
                .await
                .ok()
                .and_then(|t| serde_json::from_str::<Map<String, Value>>(&t).ok())
                .unwrap_or_default()
        };
        let covers = read(dir.join("covers.json")).await;
        let mut details = BTreeMap::new();
        if let Ok(mut rd) = tokio::fs::read_dir(dir.join("details")).await {
            while let Ok(Some(e)) = rd.next_entry().await {
                details.extend(read(e.path()).await);
            }
        }
        Self {
            dir,
            covers,
            details,
        }
    }

    async fn save(&mut self, titles: &[String]) -> Result<()> {
        // Use IGDB's cover where SteamGridDB has none.
        for t in titles {
            if self.covers.get(t).is_some_and(Value::is_null)
                && let Some(id) = self
                    .details
                    .get(t)
                    .and_then(|d| d.get("cover"))
                    .and_then(Value::as_str)
            {
                self.covers.insert(t.clone(), json!(format!("i:{id}")));
            }
        }
        let details_dir = self.dir.join("details");
        tokio::fs::create_dir_all(&details_dir).await?;
        write_atomic(
            &self.dir.join("covers.json"),
            &serde_json::to_string(&self.covers)?,
        )
        .await?;
        // Every rated game's [rating, review count] in one small file, for
        // sorting by rating.
        let ratings: BTreeMap<&String, Value> = self
            .details
            .iter()
            .filter_map(|(t, d)| {
                let r = d.get("rating")?.as_u64()?;
                let n = d.get("ratingCount").and_then(Value::as_u64).unwrap_or(0);
                Some((t, json!([r, n])))
            })
            .collect();
        write_atomic(
            &self.dir.join("ratings.json"),
            &serde_json::to_string(&ratings)?,
        )
        .await?;
        let mut buckets: BTreeMap<String, Map<String, Value>> = BTreeMap::new();
        for (t, d) in &self.details {
            buckets
                .entry(title_bucket(t))
                .or_default()
                .insert(t.clone(), d.clone());
        }
        for (b, entries) in buckets {
            write_atomic(
                &details_dir.join(format!("{b}.json")),
                &serde_json::to_string(&entries)?,
            )
            .await?;
        }
        Ok(())
    }
}

/// Write via a temp file so the app never reads a half-written file.
async fn write_atomic(path: &Path, text: &str) -> Result<()> {
    let tmp = path.with_extension("json.tmp");
    tokio::fs::write(&tmp, text).await?;
    tokio::fs::rename(&tmp, path).await?;
    Ok(())
}

async fn write_index_entry(
    base: &Path,
    slug: &str,
    spec: &ConsoleSpec,
    titles: &[String],
    data: &ConsoleData,
) -> Result<()> {
    let path = base.join("index.json");
    let mut index: Value = tokio::fs::read_to_string(&path)
        .await
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| json!({ "version": 1, "consoles": {} }));
    let count = |f: &dyn Fn(&String) -> bool| titles.iter().filter(|t| f(t)).count();
    let entry = json!({
        "name": spec.name,
        "group": spec.group,
        "folder": spec.folder,
        "listing": spec.listing,
        "igdb": spec.igdb.clone().unwrap_or_default(),
        "updated": unix_now().to_string(),
        "titles": titles.len(),
        "covers": count(&|t| data.covers.get(t).is_some_and(|v| !v.is_null())),
        "details": count(&|t| data.details.get(t).is_some_and(|v| !v.is_null())),
    });
    index["consoles"][slug] = entry;
    write_atomic(&path, &serde_json::to_string_pretty(&index)?).await
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Unix seconds -> "YYYY-MM-DD" (civil-from-days, no date crate needed).
fn ymd(secs: i64) -> String {
    let z = secs.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

struct Igdb {
    http: reqwest::Client,
    client_id: String,
    token: String,
}

impl Igdb {
    async fn login(http: &reqwest::Client, id: &str, secret: &str) -> Result<Self> {
        let resp: Value = http
            .post("https://id.twitch.tv/oauth2/token")
            .query(&[
                ("client_id", id),
                ("client_secret", secret),
                ("grant_type", "client_credentials"),
            ])
            .send()
            .await?
            .json()
            .await?;
        let token = resp["access_token"].as_str().ok_or_else(|| {
            anyhow!(
                "IGDB login failed: {}",
                resp["message"]
                    .as_str()
                    .unwrap_or("check the client ID and secret")
            )
        })?;
        Ok(Self {
            http: http.clone(),
            client_id: id.to_string(),
            token: token.to_string(),
        })
    }

    async fn query(&self, endpoint: &str, body: String) -> Result<Value> {
        for attempt in 0..6 {
            let resp = self
                .http
                .post(format!("https://api.igdb.com/v4/{endpoint}"))
                .header("Client-ID", &self.client_id)
                .bearer_auth(&self.token)
                .body(body.clone())
                .send()
                .await?;
            let status = resp.status();
            if status.as_u16() == 429 || status.is_server_error() {
                tokio::time::sleep(Duration::from_millis(1000 << attempt)).await;
                continue;
            }
            if !status.is_success() {
                bail!("IGDB {status}: {}", resp.text().await.unwrap_or_default());
            }
            return Ok(resp.json().await?);
        }
        bail!("IGDB kept rate-limiting")
    }

    async fn platform(&self, slug: &str) -> Result<u64> {
        let r = self
            .query(
                "platforms",
                format!("fields id; where slug = \"{}\";", slug.replace('"', "")),
            )
            .await?;
        r[0]["id"]
            .as_u64()
            .ok_or_else(|| anyhow!("unknown IGDB platform: {slug}"))
    }

    /// Same rules and output as igdbDetails() in scripts/igdb.ts.
    async fn details(&self, platform: u64, title: &str) -> Result<Value> {
        const FIELDS: &str = "name,alternative_names.name,summary,first_release_date,genres.name,\
            involved_companies.company.name,involved_companies.developer,involved_companies.publisher,\
            total_rating,total_rating_count,cover.image_id,artworks.image_id,screenshots.image_id";
        let query = search_query(title);
        let safe = query.replace(['"', '\\'], " ");
        let games = self
            .query(
                "games",
                format!(
                    "search \"{safe}\"; fields {FIELDS}; where platforms = ({platform}); limit 15;"
                ),
            )
            .await?;
        let mut best: Option<&Value> = None;
        let mut best_score = 0.0;
        for g in games.as_array().into_iter().flatten() {
            let alts = g["alternative_names"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|a| a["name"].as_str());
            for n in g["name"].as_str().into_iter().chain(alts) {
                let s = match_score(&query, n);
                if s > best_score {
                    best = Some(g);
                    best_score = s;
                }
            }
        }
        let Some(g) = best.filter(|_| best_score >= MATCH_THRESHOLD) else {
            return Ok(Value::Null);
        };
        let names = |key: &str| -> Vec<String> {
            g[key]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|x| x["name"].as_str().map(String::from))
                .collect()
        };
        let ids = |key: &str, max: usize| -> Vec<String> {
            g[key]
                .as_array()
                .into_iter()
                .flatten()
                .take(max)
                .filter_map(|x| x["image_id"].as_str().map(String::from))
                .collect()
        };
        let companies = g["involved_companies"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let by_role = |role: &str| -> Vec<String> {
            companies
                .iter()
                .filter(|c| c[role].as_bool() == Some(true))
                .filter_map(|c| c["company"]["name"].as_str().map(String::from))
                .collect()
        };
        let mut d = Map::new();
        d.insert("igdb".into(), g["id"].clone());
        d.insert("name".into(), g["name"].clone());
        let mut put = |k: &str, v: Value| {
            let empty = v.is_null() || v.as_array().is_some_and(Vec::is_empty);
            if !empty {
                d.insert(k.into(), v);
            }
        };
        put("summary", g["summary"].clone());
        put(
            "released",
            g["first_release_date"]
                .as_i64()
                .map_or(Value::Null, |s| json!(ymd(s))),
        );
        put("genres", json!(names("genres")));
        put("developers", json!(by_role("developer")));
        put("publishers", json!(by_role("publisher")));
        if let Some(r) = g["total_rating"].as_f64().filter(|r| *r > 0.0) {
            put("rating", json!(r.round() as u64));
            put(
                "ratingCount",
                json!(g["total_rating_count"].as_u64().unwrap_or(0)),
            );
        }
        put("cover", g["cover"]["image_id"].clone());
        put("artworks", json!(ids("artworks", 4)));
        put("screenshots", json!(ids("screenshots", 8)));
        Ok(Value::Object(d))
    }
}

/// Distinct game titles on a listing, in page order.
async fn listing_titles(http: &reqwest::Client, listing: &str) -> Result<Vec<String>> {
    let url = url::Url::parse(listing).context("bad console link")?;
    let html = http
        .get(url.clone())
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    let parsed = scrape::parse_listing(&html, &url)?;
    let mut seen = std::collections::HashSet::new();
    Ok(parsed
        .entries
        .into_iter()
        .filter(|e| e.kind == EntryKind::File)
        .map(|e| scrape::title_of(&e.name))
        .filter(|t| seen.insert(t.clone()))
        .collect())
}

impl Fetcher {
    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    /// Fetches every console in turn. `report` is called with fresh status.
    pub async fn run(
        &self,
        http: &reqwest::Client,
        covers: &Covers,
        base: &Path,
        specs: Vec<ConsoleSpec>,
        keys: Keys,
        report: &(dyn Fn(FetchStatus) + Sync),
    ) -> Result<()> {
        let igdb = if keys.igdb_id.is_empty() || keys.igdb_secret.is_empty() {
            None
        } else {
            Some(Igdb::login(http, &keys.igdb_id, &keys.igdb_secret).await?)
        };
        if igdb.is_none() && keys.sgdb.is_empty() {
            bail!("Add a SteamGridDB key or IGDB credentials first");
        }
        for (i, spec) in specs.iter().enumerate() {
            if self.cancelled() {
                break;
            }
            {
                let mut s = self.status.lock().await;
                *s = FetchStatus {
                    running: true,
                    console: spec.name.clone(),
                    queued: specs[i + 1..].iter().map(|c| c.name.clone()).collect(),
                    ..Default::default()
                };
                report(s.clone());
            }
            self.run_console(http, covers, base, spec, &keys.sgdb, igdb.as_ref(), report)
                .await
                .with_context(|| spec.name.clone())?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_console(
        &self,
        http: &reqwest::Client,
        covers: &Covers,
        base: &Path,
        spec: &ConsoleSpec,
        sgdb_key: &str,
        igdb: Option<&Igdb>,
        report: &(dyn Fn(FetchStatus) + Sync),
    ) -> Result<()> {
        let slug = listing_slug(&spec.listing);
        let titles = listing_titles(http, &spec.listing).await?;
        let data = Mutex::new(ConsoleData::load(base.join(&slug)).await);

        let (detail_todo, cover_todo): (Vec<String>, Vec<String>) = {
            let d = data.lock().await;
            let platform_ok = igdb.is_some() && spec.igdb.as_deref().is_some_and(|s| !s.is_empty());
            (
                titles
                    .iter()
                    .filter(|t| platform_ok && !d.details.contains_key(*t))
                    .cloned()
                    .collect(),
                titles
                    .iter()
                    .filter(|t| !sgdb_key.is_empty() && !d.covers.contains_key(*t))
                    .cloned()
                    .collect(),
            )
        };
        let platform = match (igdb, &spec.igdb) {
            (Some(g), Some(slug)) if !detail_todo.is_empty() => Some(g.platform(slug).await?),
            _ => None,
        };

        {
            let mut s = self.status.lock().await;
            s.total = (detail_todo.len() + cover_todo.len()) as u64;
            s.titles = titles.len() as u64;
            report(s.clone());
        }

        let last_save = Mutex::new(Instant::now());
        let step = |found_cover: bool, found_details: bool| {
            let data = &data;
            let last_save = &last_save;
            let titles = &titles;
            async move {
                let mut s = self.status.lock().await;
                s.done += 1;
                s.covers += u64::from(found_cover);
                s.details += u64::from(found_details);
                // Save every few seconds so results show up in the app as they come in.
                let mut ls = last_save.lock().await;
                if ls.elapsed() > Duration::from_secs(4) {
                    *ls = Instant::now();
                    drop(ls);
                    let _ = data.lock().await.save(titles).await;
                }
                report(s.clone());
            }
        };

        // IGDB and SteamGridDB are separate services, so run both at once.
        let details_loop = async {
            let (Some(g), Some(p)) = (igdb, platform) else {
                return Ok::<_, anyhow::Error>(());
            };
            for t in &detail_todo {
                if self.cancelled() {
                    break;
                }
                match g.details(p, t).await {
                    Ok(v) => {
                        let found = !v.is_null();
                        data.lock().await.details.insert(t.clone(), v);
                        step(false, found).await;
                    }
                    Err(e) => {
                        eprintln!("[igdb] {t}: {e:#}");
                        step(false, false).await;
                    }
                }
                tokio::time::sleep(Duration::from_millis(300)).await; // IGDB allows 4/s
            }
            Ok(())
        };
        let covers_loop = async {
            for t in &cover_todo {
                if self.cancelled() {
                    break;
                }
                match covers.fetch(t, sgdb_key).await {
                    Ok(url) => {
                        let found = url.is_some();
                        let r = url.map(|u| match u.strip_prefix(SGDB_CDN) {
                            Some(rest) => format!("s:{rest}"),
                            None => u,
                        });
                        data.lock().await.covers.insert(t.clone(), json!(r));
                        step(found, false).await;
                    }
                    Err(e) if e.to_string().contains("401") => {
                        return Err(anyhow!("SteamGridDB rejected the API key"));
                    }
                    Err(e) => {
                        eprintln!("[sgdb] {t}: {e:#}");
                        step(false, false).await;
                    }
                }
                tokio::time::sleep(Duration::from_millis(150)).await;
            }
            Ok(())
        };
        let (a, b) = tokio::join!(details_loop, covers_loop);

        let mut d = data.lock().await;
        d.save(&titles).await?;
        write_index_entry(base, &slug, spec, &titles, &d).await?;
        a.and(b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_match_typescript() {
        assert_eq!(
            listing_slug("https://minerva-archive.org/browse/Redump/Microsoft%20-%20Xbox%20360/"),
            "redump-microsoft-xbox-360"
        );
        assert_eq!(
            listing_slug("https://minerva-archive.org/browse/./Redump/"),
            "redump"
        );
        assert_eq!(
            listing_slug(
                "https://minerva-archive.org/browse/Redump/Nintendo%20-%20GameCube%20-%20NKit%20RVZ%20%5Bzstd-19-128k%5D/"
            ),
            "redump-nintendo-gamecube-nkit-rvz-zstd-19-128k"
        );
    }

    #[test]
    fn dates() {
        assert_eq!(ymd(0), "1970-01-01");
        assert_eq!(ymd(1_190_678_400), "2007-09-25"); // Halo 3's release day
    }
}

#[cfg(test)]
mod live_tests {
    use super::*;

    /// Fetches Virtual Boy for real using the project's key files:
    /// cargo test live_fetch -- --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn live_fetch() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let sgdb = std::fs::read_to_string(root.join(".steamgriddb-key"))
            .unwrap()
            .trim()
            .to_string();
        let creds = std::fs::read_to_string(root.join(".igdb-credentials")).unwrap();
        let get = |k: &str| {
            creds
                .lines()
                .find_map(|l| l.strip_prefix(&format!("{k}=")))
                .unwrap()
                .trim()
                .to_string()
        };
        let keys = Keys {
            sgdb,
            igdb_id: get("client_id"),
            igdb_secret: get("client_secret"),
        };
        let http = reqwest::Client::builder()
            .user_agent("owlcart-test")
            .build()
            .unwrap();
        let base = std::env::temp_dir().join(format!("rf-fetch-{}", std::process::id()));
        let covers = Covers::new(http.clone(), base.join("cache"));
        let spec = ConsoleSpec {
            name: "Virtual Boy".into(),
            group: "Nintendo".into(),
            folder: Some("virtualboy".into()),
            listing: "https://minerva-archive.org/browse/No-Intro/Nintendo%20-%20Virtual%20Boy/"
                .into(),
            igdb: Some("virtualboy".into()),
        };
        let f = Fetcher::default();
        f.run(&http, &covers, &base, vec![spec], keys, &|_| {})
            .await
            .unwrap();
        let s = f.status.lock().await.clone();
        println!(
            "titles={} looked up={}/{} covers found={} details found={}",
            s.titles, s.done, s.total, s.covers, s.details
        );

        // The app reads it back through the normal data path.
        let d = crate::data::DataSource::new(http, base.join("cache"));
        let b = base.to_string_lossy();
        let idx = d.consoles(&b).await.unwrap();
        println!("index: {}", idx["no-intro-nintendo-virtual-boy"]);
        let det = d
            .details(&b, "no-intro-nintendo-virtual-boy", "Mario's Tennis")
            .await
            .unwrap();
        println!(
            "Mario's Tennis -> {:?}",
            det.as_ref().map(|v| v["name"].clone())
        );
        println!(
            "cover -> {:?}",
            d.cover(&b, "no-intro-nintendo-virtual-boy", "Mario's Tennis")
                .await
                .unwrap()
        );
        assert!(s.total > 0 && s.done == s.total);
        std::fs::remove_dir_all(base).unwrap();
    }
}
