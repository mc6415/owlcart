//! Published game data (built by scripts/build-data.ts):
//!
//!   <base>/index.json                     consoles and their `updated` versions
//!   <base>/<slug>/covers.json             title -> cover ref ("s:..." / "i:..." / URL) or null
//!   <base>/<slug>/details/<00-ff>.json    title -> IGDB details or null
//!   <base>/<slug>/games.json              title -> [IGDB id] or [IGDB id, rating, review count]
//!
//! `base` is a URL or a local folder. Remote files are cached on disk and only
//! re-downloaded when the console's `updated` value in index.json changes, so
//! browsing works offline. Local files are re-read whenever they change.

use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::Mutex;

pub struct DataSource {
    http: reqwest::Client,
    cache_dir: PathBuf,
    /// Remote files by URL, with the version they were loaded for.
    remote: Mutex<HashMap<String, (String, Arc<Value>)>>,
    /// index.json per base, fetched once per session.
    indexes: Mutex<HashMap<String, Arc<Value>>>,
    /// Local files with their modified time.
    local: Mutex<HashMap<PathBuf, (SystemTime, Arc<Value>)>>,
}

fn null() -> Arc<Value> {
    Arc::new(Value::Null)
}

fn is_remote(base: &str) -> bool {
    base.starts_with("http://") || base.starts_with("https://")
}

/// Low byte of the FNV-1a hash of the title, as two hex digits.
/// Must match `titleBucket` in src/lib/names.ts.
pub fn title_bucket(title: &str) -> String {
    let mut h: u32 = 0x811c_9dc5;
    for b in title.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    format!("{:02x}", h & 0xff)
}

/// Expands a compact cover reference. Must match `coverUrl` in src/lib/names.ts.
pub fn cover_url(r: &str) -> String {
    if let Some(p) = r.strip_prefix("s:") {
        format!("https://cdn2.steamgriddb.com/{p}")
    } else if let Some(id) = r.strip_prefix("i:") {
        format!("https://images.igdb.com/igdb/image/upload/t_cover_big/{id}.jpg")
    } else {
        r.to_string()
    }
}

impl DataSource {
    pub fn new(http: reqwest::Client, cache_dir: PathBuf) -> Self {
        Self {
            http,
            cache_dir: cache_dir.join("data"),
            remote: Mutex::new(HashMap::new()),
            indexes: Mutex::new(HashMap::new()),
            local: Mutex::new(HashMap::new()),
        }
    }

    /// The `consoles` map from index.json (slug -> name, group, listing, counts).
    pub async fn consoles(&self, base: &str) -> Result<Value> {
        let base = base.trim().trim_end_matches('/');
        if base.is_empty() {
            return Ok(Value::Null);
        }
        let index = if is_remote(base) {
            self.index(base).await?
        } else {
            self.local_file(&Path::new(base).join("index.json")).await?
        };
        Ok(index.get("consoles").cloned().unwrap_or(Value::Null))
    }

    /// The whole games.json map for a console (Null if there's no data).
    pub async fn games_map(&self, base: &str, slug: &str) -> Result<Arc<Value>> {
        self.file(base, slug, "games.json").await
    }

    /// The whole covers.json map for a console (Null if there's no data).
    pub async fn covers_map(&self, base: &str, slug: &str) -> Result<Arc<Value>> {
        self.file(base, slug, "covers.json").await
    }

    /// Cover URL for a title. Outer None = the data has no entry for it.
    pub async fn cover(
        &self,
        base: &str,
        slug: &str,
        title: &str,
    ) -> Result<Option<Option<String>>> {
        let covers = self.file(base, slug, "covers.json").await?;
        Ok(covers.get(title.trim()).map(|v| v.as_str().map(cover_url)))
    }

    /// IGDB details for a title, if any.
    pub async fn details(&self, base: &str, slug: &str, title: &str) -> Result<Option<Value>> {
        let title = title.trim();
        let bucket = self
            .file(base, slug, &format!("details/{}.json", title_bucket(title)))
            .await?;
        Ok(bucket.get(title).filter(|v| !v.is_null()).cloned())
    }

    async fn file(&self, base: &str, slug: &str, rel: &str) -> Result<Arc<Value>> {
        let base = base.trim().trim_end_matches('/');
        if base.is_empty() || slug.is_empty() {
            return Ok(null());
        }
        if !is_remote(base) {
            return self.local_file(&Path::new(base).join(slug).join(rel)).await;
        }
        // The console's `updated` value says whether cached copies are current.
        let index = self.index(base).await?;
        let Some(version) = index
            .pointer(&format!("/consoles/{slug}/updated"))
            .and_then(Value::as_str)
            .map(str::to_string)
        else {
            return Ok(null()); // no data for this console
        };
        let url = format!("{base}/{slug}/{rel}");
        if let Some((v, value)) = self.remote.lock().await.get(&url)
            && *v == version
        {
            return Ok(value.clone());
        }
        let value = self.fetch_cached(&url, &version).await?;
        self.remote
            .lock()
            .await
            .insert(url, (version, value.clone()));
        Ok(value)
    }

    async fn index(&self, base: &str) -> Result<Arc<Value>> {
        if let Some(i) = self.indexes.lock().await.get(base) {
            return Ok(i.clone());
        }
        // Always try for a fresh index once per session; fall back to the cache offline.
        let url = format!("{base}/index.json");
        let value = self.fetch_cached(&url, "").await?;
        self.indexes
            .lock()
            .await
            .insert(base.to_string(), value.clone());
        Ok(value)
    }

    /// Where a remote URL is cached on disk.
    fn cache_path(&self, url: &str) -> PathBuf {
        let safe: String = url
            .split("://")
            .nth(1)
            .unwrap_or(url)
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || "-_./".contains(c) {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        self.cache_dir.join(safe.replace("..", "_"))
    }

    /// Returns the disk copy if it was saved for `version` (an empty version
    /// means always re-download), otherwise downloads it. Offline, any disk
    /// copy is better than nothing.
    async fn fetch_cached(&self, url: &str, version: &str) -> Result<Arc<Value>> {
        let path = self.cache_path(url);
        let version_path = path.with_extension("json.version");
        let cached = || async {
            let text = tokio::fs::read_to_string(&path).await.ok()?;
            serde_json::from_str::<Value>(&text).ok().map(Arc::new)
        };
        if !version.is_empty()
            && tokio::fs::read_to_string(&version_path)
                .await
                .ok()
                .as_deref()
                == Some(version)
            && let Some(v) = cached().await
        {
            return Ok(v);
        }
        let downloaded = async {
            let resp = self.http.get(url).send().await?;
            if resp.status() == reqwest::StatusCode::NOT_FOUND {
                return Ok::<_, anyhow::Error>("null".to_string());
            }
            Ok(resp.error_for_status()?.text().await?)
        }
        .await;
        match downloaded {
            Ok(text) => {
                let value: Value = serde_json::from_str(&text)?;
                if let Some(dir) = path.parent() {
                    let _ = tokio::fs::create_dir_all(dir).await;
                }
                let _ = tokio::fs::write(&path, &text).await;
                let _ = tokio::fs::write(&version_path, version).await;
                Ok(Arc::new(value))
            }
            Err(e) => match cached().await {
                Some(v) => Ok(v),
                None => Err(e),
            },
        }
    }

    async fn local_file(&self, path: &Path) -> Result<Arc<Value>> {
        let modified = match tokio::fs::metadata(path).await {
            Ok(m) => m.modified()?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(null()),
            Err(e) => return Err(e.into()),
        };
        let mut cache = self.local.lock().await;
        if let Some((when, value)) = cache.get(path)
            && *when == modified
        {
            return Ok(value.clone());
        }
        let text = tokio::fs::read_to_string(path).await?;
        match serde_json::from_str::<Value>(&text) {
            Ok(v) => {
                let v = Arc::new(v);
                cache.insert(path.to_path_buf(), (modified, v.clone()));
                Ok(v)
            }
            // Caught mid-write by the build script: keep the previous copy.
            Err(e) => match cache.get(path) {
                Some((_, v)) => Ok(v.clone()),
                None => Err(e.into()),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buckets_match_typescript() {
        // Same values as titleBucket() in src/lib/names.ts.
        assert_eq!(title_bucket("Halo 3"), "5a");
        assert_eq!(title_bucket("Ōkami"), "f6");
        assert_eq!(title_bucket(""), "c5");
    }

    #[test]
    fn cover_refs() {
        assert_eq!(
            cover_url("s:thumb/a.jpg"),
            "https://cdn2.steamgriddb.com/thumb/a.jpg"
        );
        assert_eq!(
            cover_url("i:abc"),
            "https://images.igdb.com/igdb/image/upload/t_cover_big/abc.jpg"
        );
        assert_eq!(cover_url("https://x/y.png"), "https://x/y.png");
    }

    #[tokio::test]
    async fn local_folder() {
        let dir = std::env::temp_dir().join(format!("rf-data-{}", std::process::id()));
        let slug_dir = dir.join("redump-x");
        std::fs::create_dir_all(slug_dir.join("details")).unwrap();
        std::fs::write(
            slug_dir.join("covers.json"),
            r#"{"Halo 3":"s:thumb/h.jpg","Nothing":null}"#,
        )
        .unwrap();
        std::fs::write(
            slug_dir
                .join("details")
                .join(format!("{}.json", title_bucket("Halo 3"))),
            r#"{"Halo 3":{"igdb":1,"name":"Halo 3"}}"#,
        )
        .unwrap();
        let d = DataSource::new(reqwest::Client::new(), dir.join("cache"));
        let base = dir.to_string_lossy();
        assert_eq!(
            d.cover(&base, "redump-x", "Halo 3").await.unwrap(),
            Some(Some("https://cdn2.steamgriddb.com/thumb/h.jpg".into()))
        );
        assert_eq!(
            d.cover(&base, "redump-x", "Nothing").await.unwrap(),
            Some(None)
        );
        assert_eq!(d.cover(&base, "redump-x", "Unknown").await.unwrap(), None);
        assert_eq!(d.cover(&base, "other", "Halo 3").await.unwrap(), None);
        let det = d
            .details(&base, "redump-x", "Halo 3")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(det["name"], "Halo 3");
        assert!(
            d.details(&base, "redump-x", "Unknown")
                .await
                .unwrap()
                .is_none()
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[cfg(test)]
mod remote_tests {
    use super::*;

    /// Serve data/ over HTTP and run with:
    /// DATA_URL=http://127.0.0.1:8765 cargo test remote -- --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn remote_with_disk_cache() {
        let base = std::env::var("DATA_URL").unwrap();
        let cache = std::env::temp_dir().join(format!("rf-remote-{}", std::process::id()));
        let slug = "redump-microsoft-xbox-360";

        let d = DataSource::new(reqwest::Client::new(), cache.clone());
        let cover = d.cover(&base, slug, "Halo 3").await.unwrap();
        println!("cover: {cover:?}");
        assert!(matches!(cover, Some(Some(_))));
        let det = d.details(&base, slug, "Halo 3").await.unwrap().unwrap();
        println!("details name: {}", det["name"]);

        // A fresh instance with the server unreachable must use the disk cache.
        let offline = DataSource::new(reqwest::Client::new(), cache.clone());
        let bad = "http://127.0.0.1:9";
        // Seed the offline instance's view of the index from the cached copy.
        let cached_index = d.cache_path(&format!("{base}/index.json"));
        let target = offline.cache_path(&format!("{bad}/index.json"));
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::copy(&cached_index, &target).unwrap();
        let from_disk = offline.cache_path(&format!("{base}/{slug}/covers.json"));
        let to = offline.cache_path(&format!("{bad}/{slug}/covers.json"));
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::copy(&from_disk, &to).unwrap();
        std::fs::copy(
            from_disk.with_extension("json.version"),
            to.with_extension("json.version"),
        )
        .unwrap();
        let c2 = offline.cover(bad, slug, "Halo 3").await.unwrap();
        println!("offline cover: {c2:?}");
        assert_eq!(cover, c2);
        std::fs::remove_dir_all(cache).unwrap();
    }
}
