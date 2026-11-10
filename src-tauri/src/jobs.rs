//! Download queue: resolves ROM pages, feeds the torrent engine, tracks progress and
//! moves finished files into the download folder.

use crate::engine::Engine;
use crate::covers::Covers;
use crate::data::DataSource;
use crate::fetch::{ConsoleSpec, FetchStatus, Fetcher, Keys};
use crate::scrape::{self, Listing, RomInfo};
use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::sync::{Mutex, Semaphore};
use url::Url;

/// Partially downloaded games live here (inside the download folder, so the
/// final move is a cheap rename). Each job gets its own subfolder because
/// the engine also writes fragments of neighbouring files in the torrent.
const PARTIAL_DIR: &str = ".owlcart-partial";

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub download_dir: PathBuf,
    pub max_active: usize,
    /// SteamGridDB API key for live cover lookups (optional).
    #[serde(default)]
    pub steamgriddb_key: String,
    /// IGDB (Twitch) app credentials, for fetching game details in the app.
    #[serde(default)]
    pub igdb_client_id: String,
    #[serde(default)]
    pub igdb_client_secret: String,
    /// Where in-app fetches are saved; empty means the app's own data folder.
    /// Point it at a project's data/ folder to publish what you fetch.
    #[serde(default)]
    pub fetch_dir: String,
    /// Hide games with no cover art on consoles that have cover data.
    #[serde(default)]
    pub hide_without_art: bool,
    /// Put each game in a subfolder named after its console.
    #[serde(default = "yes")]
    pub per_console_folders: bool,
    /// Unpack .zip downloads once they finish.
    #[serde(default = "yes")]
    pub extract_archives: bool,
    /// Remove the .zip after a successful extraction.
    #[serde(default = "yes")]
    pub delete_archives: bool,
    /// Where cover manifests live: a URL or local folder holding <slug>.json files.
    #[serde(default = "default_data_url", alias = "coversUrl")]
    pub data_url: String,
}

/// Published game data (covers + details). Set this once the data/ folder is hosted.
pub const DEFAULT_DATA_URL: &str = "";

fn yes() -> bool {
    true
}

fn default_data_url() -> String {
    DEFAULT_DATA_URL.to_string()
}

impl Default for Settings {
    fn default() -> Self {
        let base = dirs::download_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("."));
        Self {
            download_dir: base.join("Owlcart"),
            max_active: 3,
            steamgriddb_key: String::new(),
            igdb_client_id: String::new(),
            igdb_client_secret: String::new(),
            per_console_folders: true,
            hide_without_art: false,
            fetch_dir: String::new(),
            extract_archives: true,
            delete_archives: true,
            data_url: default_data_url(),
        }
    }
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum JobState {
    Resolving,
    Queued,
    Starting,
    Downloading,
    Paused,
    Moving,
    Extracting,
    Done,
    Failed,
    Cancelled,
}

impl JobState {
    /// Holds the torrent in the engine (one download per torrent at a time).
    fn holds_torrent(self) -> bool {
        matches!(
            self,
            Self::Starting | Self::Downloading | Self::Paused | Self::Moving
        )
    }
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub id: u64,
    pub name: String,
    pub rom_url: String,
    pub state: JobState,
    pub total: u64,
    pub completed: u64,
    pub speed: u64,
    pub peers: u64,
    pub error: Option<String>,
    pub output: Option<String>,
    #[serde(skip)]
    info: Option<RomInfo>,
    #[serde(skip)]
    torrent_url: Option<String>,
    #[serde(skip)]
    gid: Option<String>,
    #[serde(skip)]
    work_dir: Option<PathBuf>,
    #[serde(skip)]
    dest_dir: Option<PathBuf>,
    #[serde(skip)]
    folder: Option<String>,
    #[serde(skip)]
    subfolder: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct EnqueueItem {
    pub name: String,
    pub url: String,
    /// Download subfolder for this console (e.g. "psx"), if known.
    #[serde(default)]
    pub folder: Option<String>,
    /// Further subfolder inside it, e.g. "Updates" or "DLC".
    #[serde(default)]
    pub subfolder: Option<String>,
}

pub struct Manager {
    app: AppHandle,
    pub engine: Option<Engine>,
    pub engine_error: Option<String>,
    http: reqwest::Client,
    jobs: Mutex<Vec<Job>>,
    next_id: AtomicU64,
    torrents: Mutex<HashMap<String, Arc<Vec<u8>>>>,
    settings: Mutex<Settings>,
    settings_path: PathBuf,
    cache_dir: PathBuf,
    resolve_limit: Semaphore,
    covers: Covers,
    data: DataSource,
    /// Default home for data fetched in the app (Settings -> Game data).
    app_data: PathBuf,
    fetcher: Fetcher,
}

impl Manager {
    pub async fn start(
        app: AppHandle,
        config_dir: PathBuf,
        cache_dir: PathBuf,
        data_dir: PathBuf,
    ) -> Arc<Self> {
        let settings_path = config_dir.join("settings.json");
        let settings = std::fs::read(&settings_path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();

        let (engine, engine_error) = match Engine::start(cache_dir.join("engine")).await {
            Ok(e) => (Some(e), None),
            Err(e) => (None, Some(format!("{e:#}"))),
        };

        let http = reqwest::Client::builder()
            .user_agent(concat!("owlcart/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(60))
            .build()
            .expect("http client");
        Arc::new(Self {
            app,
            engine,
            engine_error,
            covers: Covers::new(http.clone(), cache_dir.clone()),
            data: DataSource::new(http.clone(), cache_dir.clone()),
            app_data: data_dir.join("data"),
            fetcher: Fetcher::default(),
            http,
            jobs: Mutex::new(Vec::new()),
            next_id: AtomicU64::new(1),
            torrents: Mutex::new(HashMap::new()),
            settings: Mutex::new(settings),
            settings_path,
            cache_dir,
            resolve_limit: Semaphore::new(4),
        })
    }

    fn engine(&self) -> Result<&Engine> {
        self.engine.as_ref().ok_or_else(|| {
            anyhow!(
                "the download engine isn't running: {}",
                self.engine_error.as_deref().unwrap_or("unknown")
            )
        })
    }

    async fn get_text(&self, url: &Url) -> Result<String> {
        let resp = self
            .http
            .get(url.clone())
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.text().await?)
    }

    pub async fn load_listing(&self, url: &str) -> Result<Listing> {
        let url = Url::parse(url.trim()).context("not a valid URL")?;
        let html = self.get_text(&url).await?;
        scrape::parse_listing(&html, &url)
    }

    // ---- settings ----

    pub async fn settings(&self) -> Settings {
        self.settings.lock().await.clone()
    }

    pub async fn set_settings(&self, s: Settings) -> Result<()> {
        if s.download_dir.as_os_str().is_empty() {
            bail!("download folder cannot be empty");
        }
        if let Some(dir) = self.settings_path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&self.settings_path, serde_json::to_vec_pretty(&s)?)?;
        *self.settings.lock().await = Settings {
            max_active: s.max_active.clamp(1, 10),
            steamgriddb_key: s.steamgriddb_key.trim().to_string(),
            igdb_client_id: s.igdb_client_id.trim().to_string(),
            igdb_client_secret: s.igdb_client_secret.trim().to_string(),
            data_url: s.data_url.trim().to_string(),
            fetch_dir: s.fetch_dir.trim().to_string(),
            ..s
        };
        Ok(())
    }

    /// Cover for a game: the published manifest first, then a live lookup
    /// if the user has their own API key.
    /// Cover for a game: the published data first, then a live lookup if
    /// the user has their own SteamGridDB key.
    pub async fn cover_art(&self, title: &str, slug: &str) -> Result<Option<String>> {
        let (key, base) = {
            let s = self.settings.lock().await;
            (s.steamgriddb_key.clone(), s.data_url.clone())
        };
        let local = self.local_data().await.to_string_lossy().into_owned();
        for b in [local.as_str(), base.as_str()] {
            if let Some(Some(url)) = self.data.cover(b, slug, title).await? {
                return Ok(Some(url));
            }
        }
        if !key.is_empty() {
            return self.covers.lookup(title, &key).await;
        }
        Ok(None)
    }

    /// Titles that have cover art on a console, from local and hosted data.
    /// None when neither has cover data for it (so nothing should be hidden).
    pub async fn cover_titles(&self, slug: &str) -> Result<Option<Vec<String>>> {
        let base = self.settings.lock().await.data_url.clone();
        let local = self.local_data().await.to_string_lossy().into_owned();
        let mut any = false;
        let mut titles = std::collections::BTreeSet::new();
        for b in [local.as_str(), base.as_str()] {
            let map = self.data.covers_map(b, slug).await.unwrap_or_default();
            if let Some(m) = map.as_object() {
                any = true;
                titles.extend(
                    m.iter()
                        .filter(|(_, v)| !v.is_null())
                        .map(|(k, _)| k.clone()),
                );
            }
        }
        Ok(any.then(|| titles.into_iter().collect()))
    }

    /// Consoles with data: hosted ones, overlaid with anything fetched locally.
    pub async fn console_index(&self) -> Result<serde_json::Value> {
        let base = self.settings.lock().await.data_url.clone();
        let mut all = self.data.consoles(&base).await.unwrap_or_default();
        let local = self
            .data
            .consoles(&self.local_data().await.to_string_lossy())
            .await?;
        if let (Some(a), Some(l)) = (all.as_object_mut(), local.as_object()) {
            a.extend(l.clone());
        } else if !local.is_null() {
            all = local;
        }
        Ok(all)
    }

    pub async fn game_details(&self, title: &str, slug: &str) -> Result<Option<serde_json::Value>> {
        let base = self.settings.lock().await.data_url.clone();
        let local = self.local_data().await.to_string_lossy().into_owned();
        if let Some(d) = self.data.details(&local, slug, title).await? {
            return Ok(Some(d));
        }
        self.data.details(&base, slug, title).await
    }

    /// Games already in the download folder for a console, as
    /// file name without extension -> path. Looks in the ES-DE folder (e.g.
    /// "psx"), the archive-named folder (e.g. "Sony - PlayStation") and the
    /// download folder itself, so older downloads are found too.
    pub async fn downloaded(
        &self,
        folder: Option<String>,
        archive_folder: Option<String>,
    ) -> Result<HashMap<String, String>> {
        let base = self.settings.lock().await.download_dir.clone();
        let mut dirs = vec![];
        for f in [
            folder.as_deref().and_then(safe_folder),
            archive_folder.as_deref().and_then(scrape::sanitize_folder),
        ]
        .into_iter()
        .flatten()
        {
            // The console folder, plus its Updates and DLC folders.
            for sub in ["", "Updates", "DLC"] {
                dirs.push(base.join(&f).join(sub));
            }
        }
        dirs.push(base);

        let mut found = HashMap::new();
        for dir in dirs {
            let Ok(mut rd) = tokio::fs::read_dir(&dir).await else {
                continue;
            };
            while let Ok(Some(e)) = rd.next_entry().await {
                let name = e.file_name().to_string_lossy().into_owned();
                if name.starts_with('.') {
                    continue;
                }
                let is_dir = e.file_type().await.map(|t| t.is_dir()).unwrap_or(false);
                // "Game (USA).zip" / "Game (USA).iso" -> "Game (USA)"; a folder keeps its name.
                let stem = if is_dir {
                    name
                } else {
                    Path::new(&name)
                        .file_stem()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or(name)
                };
                found
                    .entry(stem)
                    .or_insert_with(|| e.path().to_string_lossy().into_owned());
            }
        }
        Ok(found)
    }

    // ---- fetching game data in the app ----

    /// Folder for data fetched in the app: the user's choice, else the app's own.
    async fn local_data(&self) -> PathBuf {
        let dir = self.settings.lock().await.fetch_dir.clone();
        if dir.is_empty() {
            self.app_data.clone()
        } else {
            PathBuf::from(dir)
        }
    }

    pub async fn fetch_status(&self) -> FetchStatus {
        self.fetcher.status.lock().await.clone()
    }

    pub async fn start_fetch(self: &Arc<Self>, consoles: Vec<ConsoleSpec>) -> Result<()> {
        if consoles.is_empty() {
            bail!("no consoles to fetch");
        }
        {
            let mut s = self.fetcher.status.lock().await;
            if s.running {
                bail!("a fetch is already running");
            }
            *s = FetchStatus {
                running: true,
                console: consoles[0].name.clone(),
                ..Default::default()
            };
        }
        self.fetcher.cancel.store(false, Ordering::Relaxed);
        let keys = {
            let s = self.settings.lock().await;
            Keys {
                sgdb: s.steamgriddb_key.clone(),
                igdb_id: s.igdb_client_id.clone(),
                igdb_secret: s.igdb_client_secret.clone(),
            }
        };
        let local = self.local_data().await;
        let me = self.clone();
        tokio::spawn(async move {
            let app = me.app.clone();
            let report = move |s: FetchStatus| {
                let _ = app.emit("fetch", s);
            };
            let result = me
                .fetcher
                .run(&me.http, &me.covers, &local, consoles, keys, &report)
                .await;
            let mut s = me.fetcher.status.lock().await;
            s.running = false;
            s.queued.clear();
            s.message = Some(match result {
                Ok(()) if me.fetcher.cancelled() => "Stopped. Progress so far is saved.".into(),
                Ok(()) => "Finished.".into(),
                Err(e) => format!("{e:#}"),
            });
            let _ = me.app.emit("fetch", s.clone());
        });
        Ok(())
    }

    pub fn cancel_fetch(&self) {
        self.fetcher.cancel.store(true, Ordering::Relaxed);
    }

    // ---- job control ----

    pub async fn jobs(&self) -> Vec<Job> {
        self.jobs.lock().await.clone()
    }

    async fn emit(&self) {
        let jobs = self.jobs().await;
        let _ = self.app.emit("jobs", jobs);
    }

    pub async fn enqueue(self: &Arc<Self>, items: Vec<EnqueueItem>) {
        let mut jobs = self.jobs.lock().await;
        for item in items {
            // Skip games that are already queued or downloading.
            if jobs.iter().any(|j| {
                j.rom_url == item.url
                    && !matches!(
                        j.state,
                        JobState::Failed | JobState::Cancelled | JobState::Done
                    )
            }) {
                continue;
            }
            let id = self.next_id.fetch_add(1, Ordering::Relaxed);
            jobs.push(Job {
                id,
                name: item.name,
                rom_url: item.url,
                state: JobState::Resolving,
                total: 0,
                completed: 0,
                speed: 0,
                peers: 0,
                error: None,
                output: None,
                info: None,
                torrent_url: None,
                gid: None,
                work_dir: None,
                dest_dir: None,
                folder: item.folder.as_deref().and_then(safe_folder),
                subfolder: item.subfolder.as_deref().and_then(safe_folder),
            });
            let me = self.clone();
            tokio::spawn(async move { me.resolve(id).await });
        }
        drop(jobs);
        self.emit().await;
    }

    /// Fetches the ROM page to learn which torrent and file index to use.
    async fn resolve(self: Arc<Self>, id: u64) {
        let _permit = self.resolve_limit.acquire().await;
        let Some(rom_url) = self.with_job(id, |j| j.rom_url.clone()).await else {
            return;
        };
        let result = async {
            let url = Url::parse(&rom_url)?;
            let info = scrape::parse_rom_page(&self.get_text(&url).await?)?;
            info.file_index()?;
            let torrent_url = info.torrent_url(&url)?.to_string();
            Ok::<_, anyhow::Error>((info, torrent_url))
        }
        .await;
        self.with_job(id, |j| {
            if j.state != JobState::Resolving {
                return;
            }
            match result {
                Ok((info, torrent_url)) => {
                    j.total = info.size;
                    j.info = Some(info);
                    j.torrent_url = Some(torrent_url);
                    j.state = JobState::Queued;
                }
                Err(e) => fail(j, e),
            }
        })
        .await;
        self.emit().await;
    }

    async fn with_job<T>(&self, id: u64, f: impl FnOnce(&mut Job) -> T) -> Option<T> {
        self.jobs
            .lock()
            .await
            .iter_mut()
            .find(|j| j.id == id)
            .map(f)
    }

    pub async fn pause(&self, id: u64) -> Result<()> {
        let gid = self.with_job(id, |j| j.gid.clone()).await.flatten();
        if let Some(gid) = gid {
            self.engine()?.pause(&gid).await?;
            self.with_job(id, |j| {
                j.state = JobState::Paused;
                j.speed = 0
            })
            .await;
        }
        self.emit().await;
        Ok(())
    }

    pub async fn resume(&self, id: u64) -> Result<()> {
        let gid = self.with_job(id, |j| j.gid.clone()).await.flatten();
        if let Some(gid) = gid {
            self.engine()?.unpause(&gid).await?;
            self.with_job(id, |j| j.state = JobState::Downloading).await;
        }
        self.emit().await;
        Ok(())
    }

    pub async fn cancel(&self, id: u64) -> Result<()> {
        let taken = self
            .with_job(id, |j| {
                if matches!(j.state, JobState::Done | JobState::Moving) {
                    return None;
                }
                j.state = JobState::Cancelled;
                j.speed = 0;
                Some((j.gid.take(), j.work_dir.take()))
            })
            .await
            .flatten();
        self.emit().await;
        if let Some((gid, work_dir)) = taken {
            if let (Some(gid), Ok(engine)) = (gid, self.engine()) {
                engine.remove(&gid).await?;
            }
            if let Some(dir) = work_dir {
                let _ = tokio::fs::remove_dir_all(dir).await;
            }
        }
        Ok(())
    }

    pub async fn retry(self: &Arc<Self>, id: u64) {
        let needs_resolve = self
            .with_job(id, |j| {
                if !matches!(j.state, JobState::Failed | JobState::Cancelled) {
                    return false;
                }
                j.error = None;
                j.completed = 0;
                if j.info.is_some() {
                    j.state = JobState::Queued;
                    false
                } else {
                    j.state = JobState::Resolving;
                    true
                }
            })
            .await
            .unwrap_or(false);
        if needs_resolve {
            let me = self.clone();
            tokio::spawn(async move { me.resolve(id).await });
        }
        self.emit().await;
    }

    pub async fn clear_finished(&self) {
        self.jobs.lock().await.retain(|j| {
            !matches!(
                j.state,
                JobState::Done | JobState::Cancelled | JobState::Failed
            )
        });
        self.emit().await;
    }

    // ---- background loop ----

    pub async fn run(self: Arc<Self>) {
        loop {
            self.tick().await;
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    async fn tick(self: &Arc<Self>) {
        let Ok(engine) = self.engine() else { return };

        // 1. Refresh progress of everything the engine is working on.
        let active: Vec<(u64, String)> = self
            .jobs
            .lock()
            .await
            .iter()
            .filter(|j| matches!(j.state, JobState::Downloading | JobState::Paused))
            .filter_map(|j| Some((j.id, j.gid.clone()?)))
            .collect();
        let mut changed = !active.is_empty();
        for (id, gid) in active {
            let status = engine.status(&gid).await;
            let finished = self
                .with_job(id, |j| {
                    if !matches!(j.state, JobState::Downloading | JobState::Paused) {
                        return false;
                    }
                    match status {
                        Ok(s) => {
                            // The engine counts whole pieces, which overlap neighbouring
                            // files, so scale its progress to the game's real size.
                            let size = j.info.as_ref().map_or(s.total_length, |i| i.size);
                            j.total = size;
                            j.completed = if s.total_length > 0 {
                                (s.completed_length as u128 * size as u128 / s.total_length as u128)
                                    as u64
                            } else {
                                0
                            };
                            j.speed = s.download_speed;
                            j.peers = s.connections;
                            match s.status {
                                "complete" => {
                                    j.state = JobState::Moving;
                                    j.speed = 0;
                                    return true;
                                }
                                "error" => fail(
                                    j,
                                    anyhow!(
                                        s.error_message.unwrap_or_else(|| "download failed".into())
                                    ),
                                ),
                                "paused" => j.state = JobState::Paused,
                                "active" | "waiting" => j.state = JobState::Downloading,
                                _ => {}
                            }
                        }
                        Err(e) => fail(j, e),
                    }
                    false
                })
                .await
                .unwrap_or(false);
            if finished {
                let me = self.clone();
                tokio::spawn(async move { me.finish(id).await });
            }
        }

        // 2. Start queued jobs, one per torrent, up to max_active overall.
        let max_active = self.settings.lock().await.max_active;
        let to_start: Vec<u64> = {
            let mut jobs = self.jobs.lock().await;
            let mut busy: Vec<String> = jobs
                .iter()
                .filter(|j| j.state.holds_torrent())
                .filter_map(|j| j.torrent_url.clone())
                .collect();
            let mut picked = Vec::new();
            for j in jobs.iter_mut() {
                if busy.len() >= max_active {
                    break;
                }
                if j.state != JobState::Queued {
                    continue;
                }
                let Some(t) = &j.torrent_url else { continue };
                if busy.contains(t) {
                    continue;
                }
                busy.push(t.clone());
                j.state = JobState::Starting;
                picked.push(j.id);
            }
            picked
        };
        for id in to_start {
            changed = true;
            let me = self.clone();
            tokio::spawn(async move { me.start_job(id).await });
        }

        if changed {
            self.emit().await;
        }
    }

    async fn start_job(self: Arc<Self>, id: u64) {
        let (dest, per_console) = {
            let s = self.settings.lock().await;
            (s.download_dir.clone(), s.per_console_folders)
        };
        let Some((info, torrent_url)) = self
            .with_job(id, |j| Some((j.info.clone()?, j.torrent_url.clone()?)))
            .await
            .flatten()
        else {
            return;
        };
        let result = async {
            let torrent = self.torrent(&torrent_url).await?;
            let work_dir = dest.join(PARTIAL_DIR).join(info.id.to_string());
            tokio::fs::create_dir_all(&work_dir)
                .await
                .with_context(|| format!("cannot create {}", work_dir.display()))?;
            let gid = self
                .engine()?
                .add_torrent(&torrent, &work_dir, info.file_index()?)
                .await?;
            Ok::<_, anyhow::Error>((gid, work_dir))
        }
        .await;
        let orphan = self
            .with_job(id, |j| match result {
                Ok((gid, work_dir)) => {
                    if j.state != JobState::Starting {
                        // Cancelled while starting: clean up the new download.
                        return Some(gid);
                    }
                    j.gid = Some(gid);
                    j.work_dir = Some(work_dir);
                    // e.g. Downloads/Owlcart/psx/
                    let folder = per_console
                        .then(|| j.folder.clone().or_else(|| info.console_folder()))
                        .flatten();
                    let mut d = match folder {
                        Some(f) => dest.join(f),
                        None => dest,
                    };
                    // Updates and DLC go in their own folder: psx/Updates, psx/DLC.
                    if let Some(sub) = &j.subfolder {
                        d = d.join(sub);
                    }
                    j.dest_dir = Some(d);
                    j.state = JobState::Downloading;
                    None
                }
                Err(e) => {
                    fail(j, e);
                    None
                }
            })
            .await
            .flatten();
        if let (Some(gid), Ok(engine)) = (orphan, self.engine()) {
            let _ = engine.remove(&gid).await;
        }
        self.emit().await;
    }

    /// Downloads (or loads from cache) a .torrent file.
    async fn torrent(&self, url: &str) -> Result<Arc<Vec<u8>>> {
        let mut cache = self.torrents.lock().await;
        if let Some(t) = cache.get(url) {
            return Ok(t.clone());
        }
        let file = self.cache_dir.join("torrents").join(cache_name(url));
        let bytes = match tokio::fs::read(&file).await {
            Ok(b) => b,
            Err(_) => {
                let b = self
                    .http
                    .get(url)
                    .send()
                    .await?
                    .error_for_status()
                    .context("downloading .torrent file")?
                    .bytes()
                    .await?
                    .to_vec();
                if !b.starts_with(b"d") {
                    bail!("server did not return a valid .torrent file");
                }
                if let Some(dir) = file.parent() {
                    let _ = tokio::fs::create_dir_all(dir).await;
                }
                let _ = tokio::fs::write(&file, &b).await;
                b
            }
        };
        let bytes = Arc::new(bytes);
        cache.insert(url.to_string(), bytes.clone());
        Ok(bytes)
    }

    /// Moves the finished file into the download folder and cleans up.
    async fn finish(self: Arc<Self>, id: u64) {
        let Some((gid, work_dir, dest_dir, info)) = self
            .with_job(id, |j| {
                Some((
                    j.gid.take()?,
                    j.work_dir.clone()?,
                    j.dest_dir.clone()?,
                    j.info.clone()?,
                ))
            })
            .await
            .flatten()
        else {
            return;
        };
        if let Ok(engine) = self.engine() {
            let _ = engine.remove(&gid).await;
        }
        let result = tokio::task::spawn_blocking(move || move_out(&work_dir, &dest_dir, &info))
            .await
            .map_err(|e| anyhow!(e))
            .and_then(|r| r);
        let path = match result {
            Ok(p) => p,
            Err(e) => {
                self.with_job(id, |j| fail(j, e)).await;
                self.emit().await;
                return;
            }
        };
        let (extract, delete) = {
            let s = self.settings.lock().await;
            (s.extract_archives, s.delete_archives)
        };
        let output = if extract && crate::extract::is_zip(&path) {
            self.extract(id, path, delete).await
        } else {
            Ok(path)
        };
        self.with_job(id, |j| {
            j.state = JobState::Done;
            j.work_dir = None;
            j.speed = 0;
            match output {
                Ok(p) => {
                    j.completed = j.total;
                    j.output = Some(p.to_string_lossy().into_owned());
                }
                // The download itself is fine, so it still counts as done.
                Err((zip, e)) => {
                    j.completed = j.total;
                    j.output = Some(zip.to_string_lossy().into_owned());
                    j.error = Some(format!("Couldn't extract, so the .zip was kept: {e:#}"));
                }
            }
        })
        .await;
        self.emit().await;
    }

    /// Unpacks a finished .zip, reporting progress on the job. On failure,
    /// returns the archive path (left untouched) with the error.
    async fn extract(
        self: &Arc<Self>,
        id: u64,
        zip: PathBuf,
        delete: bool,
    ) -> std::result::Result<PathBuf, (PathBuf, anyhow::Error)> {
        use crate::extract::{Progress, extract_zip};
        use std::sync::atomic::Ordering::Relaxed;

        self.with_job(id, |j| {
            j.state = JobState::Extracting;
            j.completed = 0;
        })
        .await;
        self.emit().await;

        // Push progress to the UI twice a second while extracting.
        let progress = Arc::new(Progress::default());
        let reporter = {
            let me = self.clone();
            let p = progress.clone();
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    let (done, total) = (p.done.load(Relaxed), p.total.load(Relaxed));
                    me.with_job(id, |j| {
                        if total > 0 {
                            j.total = total;
                            j.completed = done;
                        }
                    })
                    .await;
                    me.emit().await;
                }
            })
        };
        let z = zip.clone();
        let result = tokio::task::spawn_blocking(move || extract_zip(&z, progress))
            .await
            .map_err(|e| anyhow!(e))
            .and_then(|r| r);
        reporter.abort();
        match result {
            Ok(out) => {
                if delete {
                    let _ = tokio::fs::remove_file(&zip).await;
                }
                Ok(out)
            }
            Err(e) => Err((zip, e)),
        }
    }

    pub async fn shutdown(&self) {
        if let Some(a) = &self.engine {
            a.shutdown().await;
        }
    }
}

/// A folder name from the frontend, kept to one plain path segment.
fn safe_folder(name: &str) -> Option<String> {
    let n = name.trim();
    let ok = !n.is_empty()
        && n != "."
        && n != ".."
        && n.chars().all(|c| c.is_alphanumeric() || "-_ .".contains(c));
    ok.then(|| n.to_string())
}

fn fail(j: &mut Job, e: anyhow::Error) {
    j.state = JobState::Failed;
    j.speed = 0;
    j.error = Some(format!("{e:#}"));
}

fn cache_name(url: &str) -> String {
    let last = url
        .rsplit('/')
        .next()
        .unwrap_or("torrent")
        .replace("%20", " ");
    last.chars()
        .map(|c| {
            if c.is_alphanumeric() || " .-_()".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn move_out(work_dir: &Path, dest_dir: &Path, info: &RomInfo) -> Result<PathBuf> {
    let found = walkdir::WalkDir::new(work_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .find(|e| {
            e.file_type().is_file()
                && e.file_name().to_string_lossy() == info.file_name
                && e.metadata().map(|m| m.len() == info.size).unwrap_or(false)
        })
        .map(|e| e.into_path())
        .ok_or_else(|| {
            anyhow!(
                "finished, but could not find {} in {}",
                info.file_name,
                work_dir.display()
            )
        })?;

    std::fs::create_dir_all(dest_dir)?;
    let target = unique_path(&dest_dir.join(&info.file_name));
    std::fs::rename(&found, &target)
        .or_else(|_| std::fs::copy(&found, &target).map(drop))
        .with_context(|| format!("moving file to {}", target.display()))?;
    let _ = std::fs::remove_dir_all(work_dir);
    Ok(target)
}

/// "Game.zip" -> "Game (1).zip" if it already exists.
pub(crate) fn unique_path(p: &Path) -> PathBuf {
    if !p.exists() {
        return p.to_path_buf();
    }
    let stem = p.file_stem().unwrap_or_default().to_string_lossy();
    let ext = p
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    (1..)
        .map(|n| p.with_file_name(format!("{stem} ({n}){ext}")))
        .find(|c| !c.exists())
        .expect("infinite iterator")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// End-to-end over the network. Run with:
    /// TEST_TORRENT=/path/x360.torrent cargo test e2e -- --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn e2e_single_file() {
        // RUST_LOG=librqbit=debug shows trackers, peers and pieces.
        let _ = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .try_init();
        let torrent = std::fs::read(std::env::var("TEST_TORRENT").unwrap()).unwrap();
        let base = std::env::temp_dir().join(format!("rf-e2e-{}", std::process::id()));
        let work = base.join(PARTIAL_DIR).join("1");
        std::fs::create_dir_all(&work).unwrap();
        let engine = Engine::start(base.join("engine")).await.unwrap();
        let gid = engine.add_torrent(&torrent, &work, 3064).await.unwrap();
        loop {
            let s = engine.status(&gid).await.unwrap();
            println!(
                "{} {}/{} {}B/s peers={}",
                s.status, s.completed_length, s.total_length, s.download_speed, s.connections
            );
            if s.status == "complete" {
                break;
            }
            assert_ne!(s.status, "error", "{:?}", s.error_message);
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
        engine.remove(&gid).await.unwrap();
        let info = RomInfo {
            id: 1,
            file_name: "Original Xbox Game Support - December 2005 (1.2) (USA, Canada).zip".into(),
            size: 3639798,
            torrents: None,
            so_id: Some(3064),
            full_path: None,
        };
        let out = move_out(&work, &base, &info).unwrap();
        println!("moved to {}", out.display());
        assert!(out.is_file() && !work.exists());
        engine.shutdown().await;
        std::fs::remove_dir_all(&base).unwrap();
    }
}
