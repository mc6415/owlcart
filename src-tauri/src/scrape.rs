//! Parsing for minerva-archive.org directory listings and ROM detail pages.

use anyhow::{Context, Result, anyhow};
use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub kind: EntryKind,
    pub name: String,
    /// Human-readable size as shown on the site, e.g. "6.89 GB".
    pub size: String,
    /// Approximate size in bytes, parsed from `size` (for sorting).
    pub size_bytes: u64,
    /// Absolute URL: a /browse/ page for folders, a /rom?id= page for files.
    pub url: String,
    pub rom_id: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum EntryKind {
    Folder,
    File,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Crumb {
    pub name: String,
    pub url: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Listing {
    pub url: String,
    pub crumbs: Vec<Crumb>,
    pub parent: Option<String>,
    pub entries: Vec<Entry>,
}

/// The `window.rom = {...}` object embedded in each ROM detail page.
#[derive(Deserialize, Clone, Debug)]
pub struct RomInfo {
    pub id: u64,
    pub file_name: String,
    pub size: u64,
    /// Path of the .torrent under /assets/, e.g.
    /// "Minerva_Myrient_v0.3/Minerva_Myrient - Redump - Microsoft - Xbox 360.torrent"
    pub torrents: Option<String>,
    /// 0-based index of this file inside that torrent (BEP 53 `so=`).
    pub so_id: Option<u64>,
    /// Path in the archive, e.g. "./Redump/Microsoft - Xbox 360/Halo 3 (USA).zip"
    #[serde(default)]
    pub full_path: Option<String>,
}

impl RomInfo {
    /// The archive folder this file sits in, e.g. "Microsoft - Xbox 360",
    /// made safe to use as a folder name on Windows and Linux.
    pub fn console_folder(&self) -> Option<String> {
        let path = self.full_path.as_deref()?;
        let parts: Vec<&str> = path
            .split('/')
            .filter(|p| !p.is_empty() && *p != ".")
            .collect();
        let parent = *parts.get(parts.len().checked_sub(2)?)?;
        sanitize_folder(parent)
    }

    pub fn torrent_url(&self, page_url: &Url) -> Result<Url> {
        let path = self
            .torrents
            .as_deref()
            .filter(|t| !t.is_empty())
            .ok_or_else(|| anyhow!("this file is not part of any torrent"))?;
        // Url::join percent-encodes spaces and other unsafe characters.
        Ok(page_url.join(&format!("/assets/{path}"))?)
    }

    /// This file's 0-based position in the console's torrent (BEP 53 `so=`).
    pub fn file_index(&self) -> Result<usize> {
        let so = self
            .so_id
            .ok_or_else(|| anyhow!("missing file index (so_id)"))?;
        Ok(so as usize)
    }
}

fn sel(s: &str) -> Selector {
    Selector::parse(s).expect("static selector")
}

fn text(el: ElementRef) -> String {
    el.text().collect::<String>().trim().to_string()
}

pub fn parse_listing(html: &str, page_url: &Url) -> Result<Listing> {
    let doc = Html::parse_document(html);
    let entry_sel = sel("div.entry[data-name]");
    let link_sel = sel("a[href]");
    let span_sel = sel("span");

    let mut entries = Vec::new();
    for div in doc.select(&entry_sel) {
        let mut main_link = None;
        let mut has_magnet = false;
        for a in div.select(&link_sel) {
            let href = a.value().attr("href").unwrap_or_default();
            if href.starts_with("javascript") {
                if a.value()
                    .attr("onclick")
                    .is_some_and(|o| o.contains("magnet:"))
                {
                    has_magnet = true;
                }
            } else if main_link.is_none() {
                main_link = Some((a, href.to_string()));
            }
        }
        // Entries without a magnet are leftovers (e.g. "*.partial") that
        // aren't in any torrent, so they can't be downloaded.
        let (Some((a, href)), true) = (main_link, has_magnet) else {
            continue;
        };
        let url = page_url.join(&href)?;
        let kind = if url.path().starts_with("/browse") {
            EntryKind::Folder
        } else if url.path() == "/rom" {
            EntryKind::File
        } else {
            continue;
        };
        let rom_id = url
            .query_pairs()
            .find(|(k, _)| k == "id")
            .and_then(|(_, v)| v.parse().ok());
        let size = div.select(&span_sel).next().map(text).unwrap_or_default();
        entries.push(Entry {
            kind,
            name: text(a).trim_end_matches('/').to_string(),
            size_bytes: parse_size(&size),
            size,
            url: url.to_string(),
            rom_id,
        });
    }

    let crumbs = doc
        .select(&sel("div.breadcrumbs a[href]"))
        .filter_map(|a| {
            let name = text(a);
            if name == "." {
                return None;
            }
            let url = page_url.join(a.value().attr("href")?).ok()?;
            Some(Crumb {
                name,
                url: url.to_string(),
            })
        })
        .collect();

    let parent = doc
        .select(&sel(".search_back a[href]"))
        .next()
        .and_then(|a| page_url.join(a.value().attr("href")?).ok())
        .map(|u| u.to_string());

    Ok(Listing {
        url: page_url.to_string(),
        crumbs,
        parent,
        entries,
    })
}

/// Makes an archive folder name safe to use on Windows and Linux
/// ("Sega - Mega CD & Sega CD" is fine; ":" and the like become "_").
pub fn sanitize_folder(name: &str) -> Option<String> {
    let safe: String = name
        .chars()
        .map(|c| {
            if "<>:\"/\\|?*".contains(c) || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    let safe = safe.trim().trim_end_matches('.').to_string();
    (!safe.is_empty() && safe != "..").then_some(safe)
}

/// Game title from a file name, without extension or bracketed tags:
/// "Halo 3 (USA) (En,Fr).zip" -> "Halo 3". Must match parseName() in src/lib/names.ts.
pub fn title_of(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    let base = [".zip", ".7z", ".rar", ".chd", ".iso"]
        .iter()
        .find(|ext| lower.ends_with(*ext))
        .map_or(name, |ext| &name[..name.len() - ext.len()]);
    match base.find(" (") {
        Some(i) => base[..i].to_string(),
        None => base.to_string(),
    }
}

pub fn parse_rom_page(html: &str) -> Result<RomInfo> {
    let start = html
        .find("window.rom =")
        .context("ROM details not found on page")?;
    let rest = html[start + "window.rom =".len()..].trim_start();
    // Parse just the first JSON value and ignore whatever follows it.
    serde_json::Deserializer::from_str(rest)
        .into_iter::<RomInfo>()
        .next()
        .context("ROM details not found on page")?
        .context("could not parse ROM details")
}

fn parse_size(s: &str) -> u64 {
    let mut parts = s.split_whitespace();
    let (Some(n), Some(unit)) = (parts.next(), parts.next()) else {
        return 0;
    };
    let n: f64 = n.parse().unwrap_or(0.0);
    let mult = match unit.to_ascii_uppercase().as_str() {
        "B" => 1.0,
        "KB" => 1024.0,
        "MB" => 1024.0_f64.powi(2),
        "GB" => 1024.0_f64.powi(3),
        "TB" => 1024.0_f64.powi(4),
        _ => 0.0,
    };
    (n * mult) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rom_page() {
        let html = r#"<script> window.rom = {"id":5,"file_name":"A; B.zip","size":10,"torrents":"x/y z.torrent","so_id":4234,"full_path":"./Redump/Sega - Mega CD & Sega CD/A; B.zip","magnet":"m"};
        window.addEventListener("load", () => {});</script>"#;
        let r = parse_rom_page(html).unwrap();
        assert_eq!(r.file_name, "A; B.zip");
        assert_eq!(r.file_index().unwrap(), 4234);
        assert_eq!(
            r.console_folder().as_deref(),
            Some("Sega - Mega CD & Sega CD")
        );
        let base = Url::parse("https://minerva-archive.org/rom?id=5").unwrap();
        assert_eq!(
            r.torrent_url(&base).unwrap().as_str(),
            "https://minerva-archive.org/assets/x/y%20z.torrent"
        );
    }

    /// Run with LISTING_HTML=/path/to/saved/page.html to check a real page.
    #[test]
    fn real_listing() {
        let Ok(path) = std::env::var("LISTING_HTML") else {
            return;
        };
        let html = std::fs::read_to_string(path).unwrap();
        let url =
            Url::parse("https://minerva-archive.org/browse/Redump/Microsoft%20-%20Xbox%20360/")
                .unwrap();
        let l = parse_listing(&html, &url).unwrap();
        println!(
            "entries={} crumbs={:?} parent={:?}",
            l.entries.len(),
            l.crumbs,
            l.parent
        );
        println!("{:?}", l.entries.last());
    }

    #[test]
    fn titles() {
        assert_eq!(title_of("Halo 3 (USA) (En,Fr).zip"), "Halo 3");
        assert_eq!(
            title_of("Adventures of Tintin, The - The Game (Europe).ZIP"),
            "Adventures of Tintin, The - The Game"
        );
        assert_eq!(title_of("No Tags.zip"), "No Tags");
    }

    #[test]
    fn sizes() {
        assert_eq!(parse_size("1.00 KB"), 1024);
        assert_eq!(parse_size("983.00 B"), 983);
    }
}
