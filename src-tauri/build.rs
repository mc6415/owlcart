use std::path::{Path, PathBuf};
use std::{env, fs};

fn main() {
    pack_data();
    tauri_build::build()
}

/// Packs the project's `data/` folder (covers and game details) into one
/// zstd-compressed blob that the app embeds, so releases work offline with
/// no downloads. Format, read back by src/bundled.rs:
///   "OWLD1", then for each file: path length (u32 LE), path (UTF-8, `/`),
///   data length (u64 LE), data.
fn pack_data() {
    let data = Path::new("../data");
    println!("cargo:rerun-if-changed=../data");

    let mut files = Vec::new();
    collect(data, data, &mut files);
    files.sort(); // same input -> same output

    let mut raw = b"OWLD1".to_vec();
    for (rel, path) in &files {
        let bytes = fs::read(path).expect("reading data file");
        raw.extend((rel.len() as u32).to_le_bytes());
        raw.extend(rel.as_bytes());
        raw.extend((bytes.len() as u64).to_le_bytes());
        raw.extend(&bytes);
    }
    let packed = zstd::encode_all(&raw[..], 19).expect("compressing data");
    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("data.bin.zst");
    fs::write(out, packed).expect("writing packed data");
}

fn collect(root: &Path, dir: &Path, files: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect(root, &p, files);
        } else if p.extension().is_some_and(|x| x == "json") {
            let rel = p.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
            files.push((rel, p));
        }
    }
}
