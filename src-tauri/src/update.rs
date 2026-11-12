//! Checks GitHub for a newer release of Owlcart, so the app can say when
//! there's an update. Nothing is downloaded or installed here: the app shows
//! a link to the release (or, on Android, its APK) and leaves the rest to you.

use anyhow::Result;
use serde::{Deserialize, Serialize};

const LATEST: &str = "https://api.github.com/repos/mc6415/owlcart/releases/latest";
/// Links the app may open for an update: this repo's releases, nothing else.
pub const RELEASES: &str = "https://github.com/mc6415/owlcart/releases/";

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Update {
    /// The new version, e.g. "0.7.0".
    pub version: String,
    /// Where to get it: the release page, or on Android its APK.
    pub url: String,
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

/// The latest release, if it's newer than this build.
pub async fn check(http: &reqwest::Client) -> Result<Option<Update>> {
    let release: Release = http
        .get(LATEST)
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(newer(release, env!("CARGO_PKG_VERSION"), cfg!(target_os = "android")))
}

fn newer(r: Release, current: &str, android: bool) -> Option<Update> {
    if r.draft || r.prerelease {
        return None;
    }
    let version = r.tag_name.trim_start_matches('v').to_string();
    if parse(&version)? <= parse(current)? {
        return None;
    }
    let apk = r
        .assets
        .iter()
        .find(|a| a.name.ends_with(".apk"))
        .map(|a| a.browser_download_url.clone());
    let url = match apk {
        Some(apk) if android => apk,
        _ => r.html_url,
    };
    // Only ever offer links into this repo's releases.
    url.starts_with(RELEASES).then_some(Update { version, url })
}

/// "0.7.1" -> [0, 7, 1]; anything else (like "0.7.1-beta") isn't compared.
fn parse(v: &str) -> Option<Vec<u64>> {
    v.split('.').map(|p| p.parse().ok()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(tag: &str) -> Release {
        Release {
            tag_name: tag.into(),
            html_url: format!("{RELEASES}tag/{tag}"),
            draft: false,
            prerelease: false,
            assets: vec![Asset {
                name: format!("Owlcart_{}_android_arm64.apk", &tag[1..]),
                browser_download_url: format!("{RELEASES}download/{tag}/Owlcart_{}_android_arm64.apk", &tag[1..]),
            }],
        }
    }

    #[test]
    fn only_newer_versions() {
        assert_eq!(newer(release("v0.6.0"), "0.6.0", false), None);
        assert_eq!(newer(release("v0.5.9"), "0.6.0", false), None);
        assert_eq!(
            newer(release("v0.10.0"), "0.9.3", false),
            Some(Update { version: "0.10.0".into(), url: format!("{RELEASES}tag/v0.10.0") })
        );
    }

    #[test]
    fn android_gets_the_apk() {
        let u = newer(release("v0.7.0"), "0.6.0", true).unwrap();
        assert!(u.url.ends_with("/Owlcart_0.7.0_android_arm64.apk"));
    }

    /// Asks GitHub for real (network): cargo test live_update -- --ignored
    #[tokio::test]
    #[ignore]
    async fn live_update() {
        let http = reqwest::Client::builder().user_agent("owlcart-test").build().unwrap();
        let u = check(&http).await.expect("GitHub release lookup");
        println!("update: {u:?}");
    }

    #[test]
    fn skips_drafts_and_prereleases() {
        let mut r = release("v1.0.0");
        r.prerelease = true;
        assert_eq!(newer(r, "0.6.0", false), None);
    }
}
