//! Folder picking on Android. The Kotlin side (gen/android/.../FolderPickerPlugin.kt)
//! opens the system picker; this turns its result into a normal path.
//! (Desktop uses the dialog plugin's folder picker instead.)

use tauri::{
    Runtime,
    plugin::{Builder, TauriPlugin},
};

#[cfg(target_os = "android")]
struct Picker<R: Runtime>(tauri::plugin::PluginHandle<R>);

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("folder-picker")
        .setup(|_app, _api| {
            #[cfg(target_os = "android")]
            {
                use tauri::Manager;
                let handle = _api.register_android_plugin("dev.owlcart.app", "FolderPickerPlugin")?;
                _app.manage(Picker(handle));
            }
            Ok(())
        })
        .build()
}

/// Shows the folder picker and returns the chosen folder's path, or None if
/// cancelled. Blocks until the user has chosen, so call it off the main thread.
#[cfg(target_os = "android")]
pub fn pick<R: Runtime>(app: &tauri::AppHandle<R>) -> anyhow::Result<Option<String>> {
    use tauri::Manager;

    #[derive(serde::Deserialize)]
    struct Picked {
        uri: Option<String>,
    }
    let picked: Picked = app.state::<Picker<R>>().0.run_mobile_plugin("pickFolder", ())?;
    match picked.uri {
        None => Ok(None),
        Some(uri) => tree_uri_to_path(&uri)
            .map(Some)
            .ok_or_else(|| anyhow::anyhow!("Choose a folder on the device's storage or memory card")),
    }
}

#[cfg(not(target_os = "android"))]
pub fn pick<R: Runtime>(_app: &tauri::AppHandle<R>) -> anyhow::Result<Option<String>> {
    Ok(None)
}

/// content://com.android.externalstorage.documents/tree/primary%3AROMs -> /storage/emulated/0/ROMs
/// content://com.android.externalstorage.documents/tree/1A2B-3C4D%3AGames -> /storage/1A2B-3C4D/Games
///
/// Only folders on the device's storage or a memory card map to a real path;
/// other providers (cloud storage, the Downloads shortcut) return None.
#[cfg_attr(not(target_os = "android"), allow(dead_code))] // used on Android and in tests
pub fn tree_uri_to_path(uri: &str) -> Option<String> {
    let rest = uri.strip_prefix("content://com.android.externalstorage.documents/tree/")?;
    let id = percent_encoding::percent_decode_str(rest.split('/').next()?)
        .decode_utf8()
        .ok()?;
    let (volume, path) = id.split_once(':')?;
    let base = if volume == "primary" {
        "/storage/emulated/0".to_string()
    } else {
        format!("/storage/{volume}")
    };
    let path = path.trim_matches('/');
    Some(if path.is_empty() { base } else { format!("{base}/{path}") })
}

#[cfg(test)]
mod tests {
    use super::tree_uri_to_path;

    #[test]
    fn maps_storage_folders() {
        let t = |s: &str| tree_uri_to_path(&format!("content://com.android.externalstorage.documents/tree/{s}"));
        assert_eq!(t("primary%3AROMs").as_deref(), Some("/storage/emulated/0/ROMs"));
        assert_eq!(t("primary%3AROMs%2Fpsx").as_deref(), Some("/storage/emulated/0/ROMs/psx"));
        assert_eq!(t("primary%3A").as_deref(), Some("/storage/emulated/0"));
        assert_eq!(t("1A2B-3C4D%3AGames").as_deref(), Some("/storage/1A2B-3C4D/Games"));
        assert_eq!(
            tree_uri_to_path("content://com.android.providers.downloads.documents/tree/downloads"),
            None
        );
    }
}
