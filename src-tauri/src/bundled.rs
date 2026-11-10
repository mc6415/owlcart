//! Game data (covers and details) shipped inside the app. build.rs packs the
//! project's `data/` folder into a compressed blob; on first launch of a build
//! it's unpacked into the cache folder and then read like any local data folder.

use anyhow::{Context, Result, anyhow, bail};
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

static PACKED: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/data.bin.zst"));

/// Unpacks the bundled data into `<cache>/bundled-data/<fingerprint>` (only the
/// first time for a given build) and returns that folder, or None if the app
/// was built without any data.
pub fn unpack(cache_dir: &Path) -> Result<Option<PathBuf>> {
    let root = cache_dir.join("bundled-data");
    let dir = root.join(format!("{:016x}", fingerprint(PACKED)));
    if dir.join(".complete").exists() {
        return Ok(Some(dir));
    }

    let mut raw = Vec::new();
    let mut src = PACKED;
    ruzstd::decoding::StreamingDecoder::new(&mut src)
        .map_err(|e| anyhow!("bundled data: {e}"))?
        .read_to_end(&mut raw)
        .context("decompressing bundled data")?;
    let mut body = raw
        .strip_prefix(b"OWLD1")
        .ok_or_else(|| anyhow!("bundled data has an unknown format"))?;
    if body.is_empty() {
        return Ok(None);
    }

    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir)?;
    while !body.is_empty() {
        let path_len = take_u32(&mut body)? as usize;
        let rel = std::str::from_utf8(take(&mut body, path_len)?)?;
        let data_len = take_u64(&mut body)? as usize;
        let data = take(&mut body, data_len)?;
        // Only plain relative paths: never write outside the folder.
        let rel = Path::new(rel);
        if !rel.components().all(|c| matches!(c, Component::Normal(_))) {
            bail!("bundled data has an unsafe path: {}", rel.display());
        }
        let target = dir.join(rel);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&target, data)?;
    }
    // Written last, so a half-finished unpack is redone next time.
    fs::write(dir.join(".complete"), b"")?;

    // Tidy up copies from earlier builds.
    if let Ok(entries) = fs::read_dir(&root) {
        for e in entries.flatten() {
            if e.path() != dir {
                let _ = fs::remove_dir_all(e.path());
            }
        }
    }
    Ok(Some(dir))
}

fn take<'a>(b: &mut &'a [u8], n: usize) -> Result<&'a [u8]> {
    if b.len() < n {
        bail!("bundled data is truncated");
    }
    let (head, tail) = b.split_at(n);
    *b = tail;
    Ok(head)
}

fn take_u32(b: &mut &[u8]) -> Result<u32> {
    Ok(u32::from_le_bytes(take(b, 4)?.try_into()?))
}

fn take_u64(b: &mut &[u8]) -> Result<u64> {
    Ok(u64::from_le_bytes(take(b, 8)?.try_into()?))
}

/// FNV-1a, so each distinct bundle gets its own folder.
fn fingerprint(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unpacks_the_bundled_data() {
        let cache = std::env::temp_dir().join(format!("rf-bundled-{}", std::process::id()));
        let dir = unpack(&cache).unwrap().expect("this build has data");
        let index: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(dir.join("index.json")).unwrap()).unwrap();
        let consoles = index["consoles"].as_object().unwrap();
        println!("bundled consoles: {}", consoles.len());
        assert!(!consoles.is_empty());
        // Second call reuses the unpacked copy.
        assert_eq!(unpack(&cache).unwrap(), Some(dir));
        fs::remove_dir_all(cache).unwrap();
    }
}
