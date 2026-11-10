//! The BitTorrent engine: librqbit, running inside the app. Downloads a single
//! file out of a console's torrent. (Replaces the external aria2c program, so
//! there's nothing extra to bundle or install, which also matters for Android.)

use anyhow::{Context, Result, anyhow};
use librqbit::{
    AddTorrent, AddTorrentOptions, AddTorrentResponse, DhtSessionConfig, ListenerMode,
    ListenerOptions, ManagedTorrent, Session, SessionOptions, TorrentStatsState,
    api::TorrentIdOrHash, dht::DhtPersistenceConfig,
};

type Handle = Arc<ManagedTorrent>;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct Engine {
    session: Arc<Session>,
    /// Our download ids -> librqbit's torrent id and handle.
    torrents: Mutex<HashMap<String, (usize, Handle)>>,
}

/// A download's state, in the terms the download queue uses.
#[derive(Debug, Clone)]
pub struct Status {
    /// "waiting", "active", "paused", "complete" or "error".
    pub status: &'static str,
    pub total_length: u64,
    pub completed_length: u64,
    /// Bytes per second.
    pub download_speed: u64,
    /// Connected peers.
    pub connections: u64,
    pub error_message: Option<String>,
}

impl Engine {
    /// Starts the torrent session. `scratch` is only a fallback output folder;
    /// every download names its own folder.
    pub async fn start(scratch: PathBuf) -> Result<Self> {
        tokio::fs::create_dir_all(&scratch).await.ok();
        let opts = SessionOptions {
            // The download queue tracks jobs itself, and resumes by re-adding.
            persistence: None,
            // Keep the DHT's peer table in our own cache folder. librqbit's
            // default location doesn't exist on Android.
            dht: Some(DhtSessionConfig {
                persistence: Some(DhtPersistenceConfig {
                    config_filename: Some(scratch.join("dht.json")),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            fastresume: false,
            // Accept incoming peers and speak uTP as well as TCP: many peers
            // only reach us that way, and without it we find far fewer.
            listen: Some(ListenerOptions {
                mode: ListenerMode::TcpAndUtp,
                enable_upnp_port_forwarding: true,
                ..Default::default()
            }),
            ..Default::default()
        };
        let session = Session::new_with_opts(scratch, opts)
            .await
            .context("couldn't start the torrent engine")?;
        Ok(Self {
            session,
            torrents: Mutex::new(HashMap::new()),
        })
    }

    async fn handle(&self, id: &str) -> Result<(usize, Handle)> {
        self.torrents
            .lock()
            .await
            .get(id)
            .cloned()
            .ok_or_else(|| anyhow!("unknown download"))
    }

    /// Starts downloading a single file (0-based `index`) out of `torrent` into `dir`.
    pub async fn add_torrent(&self, torrent: &[u8], dir: &Path, index: usize) -> Result<String> {
        let opts = AddTorrentOptions {
            only_files: Some(vec![index]),
            output_folder: Some(dir.to_string_lossy().into_owned()),
            // Picks up partial data already in the folder, so retries resume.
            overwrite: true,
            ..Default::default()
        };
        let resp = self
            .session
            .add_torrent(AddTorrent::from_bytes(torrent.to_vec()), Some(opts))
            .await
            .context("couldn't add the torrent")?;
        let (id, handle) = match resp {
            AddTorrentResponse::Added(id, h) | AddTorrentResponse::AlreadyManaged(id, h) => (id, h),
            AddTorrentResponse::ListOnly(_) => return Err(anyhow!("torrent was only listed")),
        };
        let key = id.to_string();
        self.torrents.lock().await.insert(key.clone(), (id, handle));
        Ok(key)
    }

    pub async fn status(&self, id: &str) -> Result<Status> {
        let (_, h) = self.handle(id).await?;
        let s = h.stats();
        let (speed, peers) = s.live.as_ref().map_or((0, 0), |l| {
            (l.download_speed.as_bytes(), u64::from(l.snapshot.peer_stats.live))
        });
        let status = match s.state {
            TorrentStatsState::Error => "error",
            _ if s.finished => "complete",
            TorrentStatsState::Paused => "paused",
            TorrentStatsState::Initializing { .. } => "waiting",
            TorrentStatsState::Live => "active",
        };
        Ok(Status {
            status,
            total_length: s.total_bytes,
            completed_length: s.progress_bytes,
            download_speed: speed,
            connections: peers,
            error_message: s.error,
        })
    }

    pub async fn pause(&self, id: &str) -> Result<()> {
        let (_, h) = self.handle(id).await?;
        self.session.pause(&h).await
    }

    pub async fn unpause(&self, id: &str) -> Result<()> {
        let (_, h) = self.handle(id).await?;
        self.session.unpause(&h).await
    }

    /// Stops a download and forgets it. Files are left for the caller to tidy up.
    pub async fn remove(&self, id: &str) -> Result<()> {
        let Some((tid, _)) = self.torrents.lock().await.remove(id) else {
            return Ok(());
        };
        self.session.delete(TorrentIdOrHash::Id(tid), false).await
    }

    pub async fn shutdown(&self) {
        self.session.stop().await;
    }
}
