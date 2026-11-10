//! Dual-screen devices (like the AYN Thor): the highlighted game on the second
//! screen. The Kotlin side (gen/android/.../SecondScreenPlugin.kt) shows the
//! page; elsewhere these do nothing.

use tauri::{
    Runtime,
    plugin::{Builder, TauriPlugin},
};

#[cfg(target_os = "android")]
struct Screen<R: Runtime>(tauri::plugin::PluginHandle<R>);

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("second-screen")
        .setup(|_app, _api| {
            #[cfg(target_os = "android")]
            {
                use tauri::Manager;
                let handle = _api.register_android_plugin("dev.owlcart.app", "SecondScreenPlugin")?;
                _app.manage(Screen(handle));
            }
            Ok(())
        })
        .build()
}

/// Whether the device has a second screen to show things on.
#[cfg(target_os = "android")]
pub fn available<R: Runtime>(app: &tauri::AppHandle<R>) -> anyhow::Result<bool> {
    use tauri::Manager;
    #[derive(serde::Deserialize)]
    struct Res {
        available: bool,
    }
    let r: Res = app.state::<Screen<R>>().0.run_mobile_plugin("available", ())?;
    Ok(r.available)
}

#[cfg(not(target_os = "android"))]
pub fn available<R: Runtime>(_app: &tauri::AppHandle<R>) -> anyhow::Result<bool> {
    Ok(false)
}

/// Sends the second screen what to show (shown on the first update).
#[cfg(target_os = "android")]
pub fn update<R: Runtime>(app: &tauri::AppHandle<R>, state: serde_json::Value) -> anyhow::Result<()> {
    use tauri::Manager;
    let _: serde_json::Value = app.state::<Screen<R>>().0.run_mobile_plugin("update", state)?;
    Ok(())
}

#[cfg(not(target_os = "android"))]
pub fn update<R: Runtime>(_app: &tauri::AppHandle<R>, _state: serde_json::Value) -> anyhow::Result<()> {
    Ok(())
}
