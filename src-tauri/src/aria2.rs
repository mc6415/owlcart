//! Runs a private aria2c instance and talks to it over JSON-RPC.

use anyhow::{Context, Result, anyhow, bail};
use base64::Engine as _;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

pub struct Aria2 {
    child: Mutex<Option<Child>>,
    endpoint: String,
    secret: String,
    http: reqwest::Client,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub status: String,
    #[serde(deserialize_with = "num")]
    pub total_length: u64,
    #[serde(deserialize_with = "num")]
    pub completed_length: u64,
    #[serde(deserialize_with = "num")]
    pub download_speed: u64,
    #[serde(default, deserialize_with = "num")]
    pub connections: u64,
    #[serde(default)]
    pub error_message: Option<String>,
}

/// aria2 returns all numbers as strings.
fn num<'de, D: serde::Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
    let s = String::deserialize(d)?;
    Ok(s.parse().unwrap_or(0))
}

/// Finds aria2c: an override env var, then the bundled sidecar next to our
/// executable, then whatever is on PATH.
pub fn find_binary() -> PathBuf {
    if let Some(p) = std::env::var_os("OWLCART_ARIA2C") {
        return p.into();
    }
    let name = format!("aria2c{}", std::env::consts::EXE_SUFFIX);
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(Path::to_path_buf))
    {
        let bundled = dir.join(&name);
        if bundled.is_file() {
            return bundled;
        }
    }
    name.into()
}

fn random_secret() -> String {
    (0..2)
        .map(|_| format!("{:016x}", RandomState::new().build_hasher().finish()))
        .collect()
}

fn free_port() -> Result<u16> {
    let l = std::net::TcpListener::bind("127.0.0.1:0")?;
    Ok(l.local_addr()?.port())
}

impl Aria2 {
    pub async fn spawn(bin: &Path) -> Result<Self> {
        let port = free_port()?;
        let secret = random_secret();
        let mut cmd = Command::new(bin);
        cmd.args([
            "--enable-rpc",
            "--rpc-listen-all=false",
            &format!("--rpc-listen-port={port}"),
            &format!("--rpc-secret={secret}"),
            // Exit automatically if this app dies without cleaning up.
            &format!("--stop-with-process={}", std::process::id()),
            // Torrents are sent base64-encoded over RPC and the big console
            // torrents are 10+ MB; aria2's default request limit is 2 MB.
            "--rpc-max-request-size=256M",
            "--seed-time=0",
            "--bt-save-metadata=false",
            "--max-concurrent-downloads=16",
            "--console-log-level=warn",
            "--summary-interval=0",
            // No preallocation: aria2 also creates the neighbouring files that
            // share boundary pieces, and preallocating would reserve their full
            // size (often several GB) on disk.
            "--file-allocation=none",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
        #[cfg(windows)]
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW

        let mut child = cmd
            .spawn()
            .with_context(|| format!("could not start aria2c ({})", bin.display()))?;

        let me = Self {
            child: Mutex::new(None),
            endpoint: format!("http://127.0.0.1:{port}/jsonrpc"),
            secret,
            http: reqwest::Client::new(),
        };

        // Wait for the RPC server to come up.
        for _ in 0..50 {
            if let Some(code) = child.try_wait()? {
                let mut err = String::new();
                if let Some(mut e) = child.stderr.take() {
                    use tokio::io::AsyncReadExt;
                    let _ = e.read_to_string(&mut err).await;
                }
                bail!("aria2c exited early ({code}): {}", err.trim());
            }
            if me.call("aria2.getVersion", vec![]).await.is_ok() {
                *me.child.lock().await = Some(child);
                return Ok(me);
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        bail!("aria2c did not start its RPC server")
    }

    async fn call(&self, method: &str, params: Vec<Value>) -> Result<Value> {
        let mut all = vec![json!(format!("token:{}", self.secret))];
        all.extend(params);
        let resp: Value = self
            .http
            .post(&self.endpoint)
            .json(&json!({"jsonrpc": "2.0", "id": "1", "method": method, "params": all}))
            .send()
            .await?
            .json()
            .await?;
        if let Some(err) = resp.get("error") {
            return Err(anyhow!(
                "aria2: {}",
                err.get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown error")
            ));
        }
        Ok(resp.get("result").cloned().unwrap_or(Value::Null))
    }

    /// Starts downloading a single file (1-based `index`) out of `torrent`.
    pub async fn add_torrent(&self, torrent: &[u8], dir: &Path, index: usize) -> Result<String> {
        let b64 = base64::engine::general_purpose::STANDARD.encode(torrent);
        let opts = json!({
            "dir": dir.to_string_lossy(),
            "select-file": index.to_string(),
        });
        let gid = self
            .call("aria2.addTorrent", vec![json!(b64), json!([]), opts])
            .await?;
        gid.as_str()
            .map(str::to_string)
            .ok_or_else(|| anyhow!("aria2 returned no download id"))
    }

    pub async fn status(&self, gid: &str) -> Result<Status> {
        let keys = json!([
            "status",
            "totalLength",
            "completedLength",
            "downloadSpeed",
            "connections",
            "errorMessage"
        ]);
        Ok(serde_json::from_value(
            self.call("aria2.tellStatus", vec![json!(gid), keys])
                .await?,
        )?)
    }

    pub async fn pause(&self, gid: &str) -> Result<()> {
        self.call("aria2.forcePause", vec![json!(gid)])
            .await
            .map(drop)
    }

    pub async fn unpause(&self, gid: &str) -> Result<()> {
        self.call("aria2.unpause", vec![json!(gid)]).await.map(drop)
    }

    /// Stops a download (if running) and forgets it.
    pub async fn remove(&self, gid: &str) -> Result<()> {
        let _ = self.call("aria2.forceRemove", vec![json!(gid)]).await;
        // forceRemove is asynchronous; wait until aria2 has let go of the files.
        for _ in 0..50 {
            match self.status(gid).await {
                Ok(s) if s.status == "active" || s.status == "paused" || s.status == "waiting" => {
                    tokio::time::sleep(Duration::from_millis(100)).await
                }
                _ => break,
            }
        }
        let _ = self
            .call("aria2.removeDownloadResult", vec![json!(gid)])
            .await;
        Ok(())
    }

    pub async fn shutdown(&self) {
        let _ = self.call("aria2.forceShutdown", vec![]).await;
        if let Some(mut child) = self.child.lock().await.take() {
            let _ = tokio::time::timeout(Duration::from_secs(3), child.wait()).await;
            let _ = child.kill().await;
        }
    }
}
