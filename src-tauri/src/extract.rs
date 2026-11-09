//! Extracts downloaded .zip archives into the console folder.

use anyhow::{Context, Result, bail};
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Bytes written so far and the total to write, for progress reporting.
#[derive(Default)]
pub struct Progress {
    pub done: AtomicU64,
    pub total: AtomicU64,
}

pub fn is_zip(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("zip"))
}

/// Extracts `archive` next to itself. A single file goes straight into the
/// folder; several files (e.g. a .cue and its .bin tracks) go into a subfolder
/// named after the archive. Returns the extracted file or folder.
///
/// Each file's CRC is checked as it's read. On any error, everything written so
/// far is removed and the archive is left untouched.
pub fn extract_zip(archive: &Path, progress: Arc<Progress>) -> Result<PathBuf> {
    let dir = archive.parent().context("archive has no folder")?;
    let file = File::open(archive).with_context(|| format!("opening {}", archive.display()))?;
    let mut zip = zip::ZipArchive::new(BufReader::new(file)).context("not a valid zip file")?;

    // Plan the output paths first, rejecting anything that would escape the folder.
    let mut entries = Vec::new();
    for i in 0..zip.len() {
        let f = zip.by_index(i)?;
        if f.is_dir() {
            continue;
        }
        let Some(name) = f.enclosed_name() else {
            bail!(
                "unsafe path in archive: {}",
                f.name().map(|n| n.into_owned()).unwrap_or_default()
            );
        };
        entries.push((i, name, f.size()));
    }
    if entries.is_empty() {
        bail!("archive is empty");
    }
    progress
        .total
        .store(entries.iter().map(|e| e.2).sum(), Ordering::Relaxed);

    let single = entries.len() == 1;
    let root = if single {
        dir.to_path_buf()
    } else {
        let stem = archive
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "extracted".into());
        crate::jobs::unique_path(&dir.join(stem))
    };

    let mut written: Vec<PathBuf> = Vec::new();
    let result = (|| -> Result<PathBuf> {
        if !single {
            std::fs::create_dir_all(&root)?;
            written.push(root.clone());
        }
        let mut first = None;
        let mut buf = vec![0u8; 1 << 20];
        for (i, name, _) in &entries {
            let target = if single {
                // Keep just the file name for a lone file.
                crate::jobs::unique_path(&root.join(name.file_name().context("bad file name")?))
            } else {
                root.join(name)
            };
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut out = BufWriter::new(
                File::create(&target).with_context(|| format!("creating {}", target.display()))?,
            );
            written.push(target.clone());
            let mut entry = zip.by_index(*i)?;
            loop {
                // Reading to the end also verifies the entry's CRC.
                let n = entry
                    .read(&mut buf)
                    .with_context(|| format!("reading {}", name.display()))?;
                if n == 0 {
                    break;
                }
                out.write_all(&buf[..n])?;
                progress.done.fetch_add(n as u64, Ordering::Relaxed);
            }
            out.flush()?;
            first.get_or_insert(target);
        }
        Ok(if single {
            first.expect("one entry")
        } else {
            root.clone()
        })
    })();

    if result.is_err() {
        // Remove partial output, files first and then the folder.
        for p in written.iter().rev() {
            if p.is_dir() {
                let _ = std::fs::remove_dir_all(p);
            } else {
                let _ = std::fs::remove_file(p);
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use zip::write::SimpleFileOptions;

    fn make_zip(path: &Path, files: &[(&str, &[u8])]) {
        let mut w = zip::ZipWriter::new(File::create(path).unwrap());
        for (name, data) in files {
            w.start_file(*name, SimpleFileOptions::default()).unwrap();
            w.write_all(data).unwrap();
        }
        w.finish().unwrap();
    }

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("rf-extract-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn single_file_goes_into_folder() {
        let d = tmp("single");
        let z = d.join("Game (USA).zip");
        make_zip(&z, &[("Game (USA).iso", b"disc image")]);
        let p = Arc::new(Progress::default());
        let out = extract_zip(&z, p.clone()).unwrap();
        assert_eq!(out, d.join("Game (USA).iso"));
        assert_eq!(std::fs::read(&out).unwrap(), b"disc image");
        assert_eq!(p.done.load(Ordering::Relaxed), 10);
        std::fs::remove_dir_all(d).unwrap();
    }

    #[test]
    fn multi_file_gets_subfolder() {
        let d = tmp("multi");
        let z = d.join("Game (Europe).zip");
        make_zip(
            &z,
            &[
                ("Game (Europe).cue", b"cue"),
                ("Game (Europe) (Track 1).bin", b"bin1"),
            ],
        );
        let out = extract_zip(&z, Arc::default()).unwrap();
        assert_eq!(out, d.join("Game (Europe)"));
        assert!(out.join("Game (Europe).cue").is_file());
        assert!(out.join("Game (Europe) (Track 1).bin").is_file());
        std::fs::remove_dir_all(d).unwrap();
    }

    #[test]
    fn rejects_path_escape() {
        let d = tmp("escape");
        let z = d.join("evil.zip");
        make_zip(&z, &[("../evil.txt", b"x")]);
        assert!(extract_zip(&z, Arc::default()).is_err());
        assert!(!d.parent().unwrap().join("evil.txt").exists());
        std::fs::remove_dir_all(d).unwrap();
    }

    #[test]
    fn corrupt_archive_cleans_up() {
        let d = tmp("corrupt");
        let z = d.join("Bad.zip");
        make_zip(&z, &[("a.bin", &[7u8; 4096]), ("b.bin", &[9u8; 4096])]);
        // Flip a byte inside the first entry's data so its CRC check fails.
        let mut bytes = std::fs::read(&z).unwrap();
        let pos = bytes.windows(5).position(|w| w == b"a.bin").unwrap() + 5 + 10;
        bytes[pos] ^= 0xff;
        std::fs::write(&z, bytes).unwrap();
        assert!(extract_zip(&z, Arc::default()).is_err());
        assert!(!d.join("Bad").exists(), "partial output should be removed");
        assert!(z.exists(), "archive kept");
        std::fs::remove_dir_all(d).unwrap();
    }
}

#[cfg(test)]
mod real_tests {
    use super::*;

    /// Extracts a real archive (copied to a temp folder first):
    /// REAL_ZIP=/path/to/game.zip cargo test real_zip -- --ignored --nocapture
    #[test]
    #[ignore]
    fn real_zip() {
        let src = PathBuf::from(std::env::var("REAL_ZIP").unwrap());
        let d = std::env::temp_dir().join(format!("rf-realzip-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let z = d.join(src.file_name().unwrap());
        std::fs::copy(&src, &z).unwrap();
        let p = Arc::new(Progress::default());
        let out = extract_zip(&z, p.clone()).unwrap();
        println!(
            "extracted {} bytes of {} -> {}",
            p.done.load(Ordering::Relaxed),
            p.total.load(Ordering::Relaxed),
            out.display()
        );
        for e in walkdir::WalkDir::new(&out)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            println!(
                "  {} ({} bytes)",
                e.path().display(),
                e.metadata().unwrap().len()
            );
        }
        std::fs::remove_dir_all(d).unwrap();
    }
}
