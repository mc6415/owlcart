mod bundled;
mod covers;
mod data;
mod engine;
mod extract;
mod fetch;
mod folder_picker;
mod second_screen;
mod jobs;
mod scrape;

use jobs::{EnqueueItem, Job, Manager, Settings};
use scrape::Listing;
use std::sync::Arc;
use tauri::{Manager as _, RunEvent, State};
use tauri_plugin_opener::OpenerExt;

type Mgr = Arc<Manager>;
type Res<T> = Result<T, String>;

fn err(e: anyhow::Error) -> String {
    format!("{e:#}")
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct EngineStatus {
    ok: bool,
    error: Option<String>,
}

#[tauri::command]
fn engine_status(mgr: State<'_, Mgr>) -> EngineStatus {
    EngineStatus {
        ok: mgr.engine.is_some(),
        error: mgr.engine_error.clone(),
    }
}

#[tauri::command]
async fn load_listing(mgr: State<'_, Mgr>, url: String) -> Res<Listing> {
    mgr.load_listing(&url).await.map_err(err)
}

#[tauri::command]
async fn enqueue(mgr: State<'_, Mgr>, items: Vec<EnqueueItem>) -> Res<()> {
    mgr.enqueue(items).await;
    Ok(())
}

#[tauri::command]
async fn list_jobs(mgr: State<'_, Mgr>) -> Res<Vec<Job>> {
    Ok(mgr.jobs().await)
}

#[tauri::command]
async fn pause_job(mgr: State<'_, Mgr>, id: u64) -> Res<()> {
    mgr.pause(id).await.map_err(err)
}

#[tauri::command]
async fn resume_job(mgr: State<'_, Mgr>, id: u64) -> Res<()> {
    mgr.resume(id).await.map_err(err)
}

#[tauri::command]
async fn cancel_job(mgr: State<'_, Mgr>, id: u64) -> Res<()> {
    mgr.cancel(id).await.map_err(err)
}

#[tauri::command]
async fn retry_job(mgr: State<'_, Mgr>, id: u64) -> Res<()> {
    mgr.retry(id).await;
    Ok(())
}

#[tauri::command]
async fn clear_finished(mgr: State<'_, Mgr>) -> Res<()> {
    mgr.clear_finished().await;
    Ok(())
}

#[tauri::command]
async fn get_settings(mgr: State<'_, Mgr>) -> Res<Settings> {
    Ok(mgr.settings().await)
}

#[tauri::command]
async fn set_settings(mgr: State<'_, Mgr>, settings: Settings) -> Res<()> {
    mgr.set_settings(settings).await.map_err(err)
}

#[tauri::command]
async fn cover_art(mgr: State<'_, Mgr>, title: String, slug: String) -> Res<Option<String>> {
    mgr.cover_art(&title, &slug).await.map_err(err)
}

#[tauri::command]
async fn start_fetch(mgr: State<'_, Mgr>, consoles: Vec<fetch::ConsoleSpec>) -> Res<()> {
    mgr.start_fetch(consoles).await.map_err(err)
}

#[tauri::command]
fn cancel_fetch(mgr: State<'_, Mgr>) {
    mgr.cancel_fetch();
}

#[tauri::command]
async fn fetch_status(mgr: State<'_, Mgr>) -> Res<fetch::FetchStatus> {
    Ok(mgr.fetch_status().await)
}

#[tauri::command]
async fn cover_titles(mgr: State<'_, Mgr>, slug: String) -> Res<Option<Vec<String>>> {
    mgr.cover_titles(&slug).await.map_err(err)
}

#[tauri::command]
async fn downloaded(
    mgr: State<'_, Mgr>,
    folder: Option<String>,
    archive_folder: Option<String>,
) -> Res<std::collections::HashMap<String, String>> {
    mgr.downloaded(folder, archive_folder).await.map_err(err)
}

#[tauri::command]
async fn console_index(mgr: State<'_, Mgr>) -> Res<serde_json::Value> {
    mgr.console_index().await.map_err(err)
}

#[tauri::command]
async fn game_details(
    mgr: State<'_, Mgr>,
    title: String,
    slug: String,
) -> Res<Option<serde_json::Value>> {
    mgr.game_details(&title, &slug).await.map_err(err)
}

/// Whether there's a second screen (dual-screen devices like the AYN Thor).
#[tauri::command]
async fn second_screen_available(app: tauri::AppHandle) -> Res<bool> {
    tauri::async_runtime::spawn_blocking(move || second_screen::available(&app))
        .await
        .map_err(|e| e.to_string())?
        .map_err(err)
}

/// What to show on the second screen: the highlighted game and downloads.
#[tauri::command]
async fn second_screen_update(app: tauri::AppHandle, state: serde_json::Value) -> Res<()> {
    tauri::async_runtime::spawn_blocking(move || second_screen::update(&app, state))
        .await
        .map_err(|e| e.to_string())?
        .map_err(err)
}

/// Android's folder picker; returns the chosen folder's path (None if cancelled).
#[tauri::command]
async fn pick_folder(app: tauri::AppHandle) -> Res<Option<String>> {
    // Waits for the user, so keep it off the async threads.
    tauri::async_runtime::spawn_blocking(move || folder_picker::pick(&app))
        .await
        .map_err(|e| e.to_string())?
        .map_err(err)
}

#[tauri::command]
async fn open_downloads(app: tauri::AppHandle, mgr: State<'_, Mgr>) -> Res<()> {
    let dir = mgr.settings().await.download_dir;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    app.opener()
        .open_path(dir.to_string_lossy(), None::<&str>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn reveal_file(app: tauri::AppHandle, path: String) -> Res<()> {
    app.opener()
        .reveal_item_in_dir(path)
        .map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(folder_picker::init())
        .plugin(second_screen::init())
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            let cache_dir = app.path().app_cache_dir()?;
            let data_dir = app.path().app_data_dir()?;
            let mgr = tauri::async_runtime::block_on(Manager::start(
                app.handle().clone(),
                config_dir,
                cache_dir,
                data_dir,
            ));
            app.manage(mgr.clone());
            tauri::async_runtime::spawn(mgr.run());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            engine_status,
            load_listing,
            enqueue,
            list_jobs,
            pause_job,
            resume_job,
            cancel_job,
            retry_job,
            clear_finished,
            get_settings,
            set_settings,
            open_downloads,
            reveal_file,
            cover_art,
            game_details,
            console_index,
            downloaded,
            cover_titles,
            start_fetch,
            cancel_fetch,
            fetch_status,
            pick_folder,
            second_screen_available,
            second_screen_update,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let RunEvent::Exit = event
                && let Some(mgr) = app.try_state::<Mgr>()
            {
                tauri::async_runtime::block_on(mgr.shutdown());
            }
        });
}
