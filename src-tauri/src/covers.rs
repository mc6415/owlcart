//! Live SteamGridDB cover lookups with the user's own API key, cached on disk
//! by title. Published covers come from the data source instead (data.rs).

use anyhow::{Result, bail};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{Mutex, OnceCell, Semaphore};

const API: &str = "https://www.steamgriddb.com/api/v2";

pub struct Covers {
    http: reqwest::Client,
    cache_path: PathBuf,
    /// title -> cover URL (None = looked up, nothing found).
    known: Mutex<HashMap<String, Option<String>>>,
    /// In-flight lookups, so several cards with the same title share one request.
    pending: Mutex<HashMap<String, Arc<OnceCell<Option<String>>>>>,
    limit: Semaphore,
}

#[derive(Deserialize)]
struct Resp<T> {
    success: bool,
    // A missing `data` field becomes None; no `default` attribute so T needn't be Default.
    data: Option<T>,
    #[serde(default)]
    errors: Vec<String>,
}

#[derive(Deserialize)]
struct Game {
    id: u64,
    name: String,
}

#[derive(Deserialize)]
struct Grid {
    url: String,
    #[serde(default)]
    thumb: Option<String>,
}

impl Covers {
    pub fn new(http: reqwest::Client, cache_dir: PathBuf) -> Self {
        let cache_path = cache_dir.join("covers.json");
        let known = std::fs::read(&cache_path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        Self {
            http,
            cache_path,
            known: Mutex::new(known),
            pending: Mutex::new(HashMap::new()),
            limit: Semaphore::new(4),
        }
    }

    pub async fn lookup(&self, title: &str, key: &str) -> Result<Option<String>> {
        let title = title.trim();
        if let Some(hit) = self.known.lock().await.get(title) {
            return Ok(hit.clone());
        }
        if key.is_empty() || title.is_empty() {
            return Ok(None);
        }
        let cell = self
            .pending
            .lock()
            .await
            .entry(title.to_string())
            .or_default()
            .clone();
        let result = cell
            .get_or_try_init(|| async {
                let _permit = self.limit.acquire().await?;
                self.fetch(title, key).await
            })
            .await
            .cloned();
        self.pending.lock().await.remove(title);

        // Only remember definite answers; network errors get retried next time.
        let found = result?;
        let mut known = self.known.lock().await;
        known.insert(title.to_string(), found.clone());
        if let Ok(json) = serde_json::to_vec(&*known) {
            drop(known);
            if let Some(dir) = self.cache_path.parent() {
                let _ = tokio::fs::create_dir_all(dir).await;
            }
            let _ = tokio::fs::write(&self.cache_path, json).await;
        }
        Ok(found)
    }

    async fn get<T: for<'de> Deserialize<'de>>(
        &self,
        url: url::Url,
        key: &str,
    ) -> Result<Option<T>> {
        let resp = self.http.get(url).bearer_auth(key).send().await?;
        let status = resp.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let body: Resp<T> = resp.json().await?;
        if !body.success {
            bail!("SteamGridDB ({status}): {}", body.errors.join(", "));
        }
        Ok(body.data)
    }

    pub(crate) async fn fetch(&self, title: &str, key: &str) -> Result<Option<String>> {
        let query = search_query(title);
        let mut url = url::Url::parse(API)?;
        url.path_segments_mut()
            .map_err(|_| anyhow::anyhow!("bad url"))?
            .extend(["search", "autocomplete", &query]);
        let games: Vec<Game> = self.get(url, key).await?.unwrap_or_default();

        // Take the closest name; no cover beats the wrong cover.
        let best = games
            .iter()
            .map(|g| (match_score(&query, &g.name), g))
            .max_by(|a, b| a.0.total_cmp(&b.0));
        let Some((score, game)) = best else {
            return Ok(None);
        };
        if score < MATCH_THRESHOLD {
            return Ok(None);
        }

        let url = url::Url::parse_with_params(
            &format!("{API}/grids/game/{}", game.id),
            &[
                ("dimensions", "600x900,342x482,660x930"),
                ("types", "static"),
                ("nsfw", "false"),
                ("humor", "false"),
                ("epilepsy", "false"),
            ],
        )?;
        let grids: Vec<Grid> = self.get(url, key).await?.unwrap_or_default();
        Ok(grids.into_iter().next().map(|g| g.thumb.unwrap_or(g.url)))
    }
}

// Matching rules. Keep in sync with src/lib/names.ts (used by scripts/build-covers.ts).

pub(crate) const MATCH_THRESHOLD: f64 = 0.85;

/// "Adventures of Tintin, The - The Game" -> "The Adventures of Tintin: The Game"
pub(crate) fn search_query(title: &str) -> String {
    title
        .split(" - ")
        .map(|part| {
            for article in ["The", "A", "An"] {
                if let Some(rest) = part.strip_suffix(&format!(", {article}")) {
                    return format!("{article} {rest}");
                }
            }
            part.to_string()
        })
        .collect::<Vec<_>>()
        .join(": ")
}

fn numbers(w: &[String]) -> Vec<&String> {
    let mut n: Vec<&String> = w
        .iter()
        .filter(|x| x.chars().all(|c| c.is_ascii_digit()))
        .collect();
    n.sort();
    n
}

fn words(s: &str) -> Vec<String> {
    s.to_lowercase()
        .replace('&', " and ")
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

/// Dice overlap of words, 0..1; 0 if the numbers differ ("Wipeout 2" vs "Wipeout").
pub(crate) fn match_score(query: &str, candidate: &str) -> f64 {
    use std::collections::BTreeSet;
    let (a, b) = (words(query), words(candidate));
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    if numbers(&a) != numbers(&b) {
        return 0.0;
    }
    let (sa, sb): (BTreeSet<_>, BTreeSet<_>) = (a.iter().collect(), b.iter().collect());
    2.0 * sa.intersection(&sb).count() as f64 / (sa.len() + sb.len()) as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching() {
        assert_eq!(
            search_query("Adventures of Tintin, The - The Game"),
            "The Adventures of Tintin: The Game"
        );
        assert_eq!(match_score("ABC Wipeout 2", "ABC Wipeout"), 0.0);
        assert_eq!(match_score("Halo 3", "Halo 3"), 1.0);
        assert!((match_score("Halo 3", "Halo 3: ODST") - 0.8).abs() < 1e-9);
        assert_eq!(
            match_score("AC-DC Live: Rock Band", "AC/DC Live: Rock Band"),
            1.0
        );
        assert!(match_score("Agarest Senki Zero: Dawn of War", "Agarest Zero") < MATCH_THRESHOLD);
    }
}
