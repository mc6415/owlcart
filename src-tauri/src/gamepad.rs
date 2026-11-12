//! Whether a controller is connected, and rumble (Android). The Kotlin side
//! (gen/android/.../GamepadPlugin.kt) checks the input devices and runs the
//! vibrator; elsewhere these say no and do nothing.

use tauri::{
    Runtime,
    plugin::{Builder, TauriPlugin},
};

#[cfg(target_os = "android")]
struct Pad<R: Runtime>(tauri::plugin::PluginHandle<R>);

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("gamepad")
        .setup(|_app, _api| {
            #[cfg(target_os = "android")]
            {
                use tauri::Manager;
                let handle = _api.register_android_plugin("dev.owlcart.app", "GamepadPlugin")?;
                _app.manage(Pad(handle));
            }
            Ok(())
        })
        .build()
}

#[cfg(target_os = "android")]
pub fn connected<R: Runtime>(app: &tauri::AppHandle<R>) -> anyhow::Result<bool> {
    use tauri::Manager;
    #[derive(serde::Deserialize)]
    struct Res {
        connected: bool,
    }
    let r: Res = app.state::<Pad<R>>().0.run_mobile_plugin("connected", ())?;
    Ok(r.connected)
}

#[cfg(target_os = "android")]
pub fn rumble<R: Runtime>(app: &tauri::AppHandle<R>, ms: u64, strength: u8) -> anyhow::Result<()> {
    use tauri::Manager;
    #[derive(serde::Serialize)]
    struct Args {
        ms: u64,
        strength: u8,
    }
    app.state::<Pad<R>>().0.run_mobile_plugin::<serde_json::Value>("rumble", Args { ms, strength })?;
    Ok(())
}

#[cfg(not(target_os = "android"))]
pub fn rumble<R: Runtime>(_app: &tauri::AppHandle<R>, _ms: u64, _strength: u8) -> anyhow::Result<()> {
    Ok(())
}

#[cfg(not(target_os = "android"))]
pub fn connected<R: Runtime>(_app: &tauri::AppHandle<R>) -> anyhow::Result<bool> {
    Ok(false)
}
