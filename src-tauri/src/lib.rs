mod app_chrome;
mod errors;
mod firefox;
mod icons;
mod models;
mod protocol;
mod shortcuts;
mod storage;
mod taskbar_tabs;
mod windows;

use errors::AppError;
use models::{AppSettings, CreateShortcutRequest, ScanResult, ShortcutView};
use shortcuts as shortcut_service;
use storage::AppPaths;
use tauri::{AppHandle, Manager};

fn app_paths(app: &AppHandle) -> Result<AppPaths, AppError> {
    Ok(AppPaths {
        config_dir: app.path().app_config_dir().map_err(|error| {
            AppError::new(
                "application_data_unavailable",
                "The application settings directory could not be accessed.",
                Some(error.to_string()),
            )
        })?,
        data_dir: app.path().app_data_dir().map_err(|error| {
            AppError::new(
                "application_data_unavailable",
                "The application data directory could not be accessed.",
                Some(error.to_string()),
            )
        })?,
        cache_dir: app.path().app_cache_dir().map_err(|error| {
            AppError::new(
                "application_cache_unavailable",
                "The application icon cache directory could not be accessed.",
                Some(error.to_string()),
            )
        })?,
    })
}

async fn run_blocking<T: Send + 'static>(
    code: &str,
    message: &str,
    operation: impl FnOnce() -> Result<T, AppError> + Send + 'static,
) -> Result<T, AppError> {
    tauri::async_runtime::spawn_blocking(operation)
        .await
        .map_err(|error| AppError::new(code, message, Some(error.to_string())))?
}

#[tauri::command]
fn get_settings(app: AppHandle) -> Result<AppSettings, AppError> {
    let paths = app_paths(&app)?;
    storage::load_settings(&paths.config_dir)
}

#[tauri::command]
fn save_settings(app: AppHandle, settings: AppSettings) -> Result<(), AppError> {
    let paths = app_paths(&app)?;
    storage::save_settings(&paths.config_dir, &settings)
}

#[tauri::command]
async fn scan_firefox(app: AppHandle) -> Result<ScanResult, AppError> {
    let paths = app_paths(&app)?;
    let settings = storage::load_settings(&paths.config_dir)?;
    run_blocking("scan_failed", "Firefox could not be scanned.", move || {
        Ok(firefox::scan_firefox(&settings))
    })
    .await
}

#[tauri::command]
async fn list_managed_shortcuts(app: AppHandle) -> Result<Vec<ShortcutView>, AppError> {
    let paths = app_paths(&app)?;
    let settings = storage::load_settings(&paths.config_dir)?;
    run_blocking(
        "shortcut_list_failed",
        "Managed shortcuts could not be loaded.",
        move || shortcut_service::list_managed_shortcuts(&paths, &settings),
    )
    .await
}

#[tauri::command]
async fn create_shortcut(
    app: AppHandle,
    request: CreateShortcutRequest,
) -> Result<ShortcutView, AppError> {
    let paths = app_paths(&app)?;
    run_blocking(
        "shortcut_create_failed",
        "The Windows shortcut could not be created.",
        move || shortcut_service::create_managed_shortcut(&paths, request),
    )
    .await
}

#[tauri::command]
async fn update_shortcut(
    app: AppHandle,
    shortcut_id: String,
    request: CreateShortcutRequest,
) -> Result<ShortcutView, AppError> {
    let paths = app_paths(&app)?;
    run_blocking(
        "shortcut_update_failed",
        "The Windows shortcut could not be updated.",
        move || shortcut_service::update_managed_shortcut(&paths, &shortcut_id, request),
    )
    .await
}

#[tauri::command]
async fn refresh_shortcut_icon(
    app: AppHandle,
    shortcut_id: String,
) -> Result<ShortcutView, AppError> {
    let paths = app_paths(&app)?;
    run_blocking(
        "icon_refresh_failed",
        "The website icon could not be refreshed.",
        move || shortcut_service::refresh_managed_shortcut_icon(&paths, &shortcut_id),
    )
    .await
}

#[tauri::command]
async fn delete_shortcut(app: AppHandle, shortcut_id: String) -> Result<(), AppError> {
    let paths = app_paths(&app)?;
    run_blocking(
        "shortcut_delete_failed",
        "The Windows shortcut could not be deleted.",
        move || shortcut_service::delete_managed_shortcut(&paths, &shortcut_id),
    )
    .await
}

#[tauri::command]
async fn launch_shortcut(app: AppHandle, shortcut_id: String) -> Result<(), AppError> {
    let paths = app_paths(&app)?;
    run_blocking(
        "shortcut_launch_failed",
        "The Windows shortcut could not be launched.",
        move || shortcut_service::launch_managed_shortcut(&paths, &shortcut_id),
    )
    .await
}

#[tauri::command]
async fn open_shortcut_location(app: AppHandle, shortcut_id: String) -> Result<(), AppError> {
    let paths = app_paths(&app)?;
    run_blocking(
        "folder_open_failed",
        "The shortcut location could not be opened.",
        move || shortcut_service::open_shortcut_location(&paths, &shortcut_id),
    )
    .await
}

#[tauri::command]
fn open_start_menu_folder() -> Result<(), AppError> {
    shortcut_service::open_start_menu_folder()
}

#[tauri::command]
async fn open_extension_page(
    app: AppHandle,
    installation_id: String,
    profile_id: String,
    url: String,
) -> Result<(), AppError> {
    let paths = app_paths(&app)?;
    run_blocking(
        "firefox_launch_failed",
        "Firefox could not open the extension page.",
        move || shortcut_service::open_extension_page(&paths, &installation_id, &profile_id, &url),
    )
    .await
}

#[tauri::command]
fn app_chrome_status(app: AppHandle, profile_id: String) -> Result<bool, AppError> {
    let profile = chrome_profile(&app, &profile_id)?;
    app_chrome::is_enabled(std::path::Path::new(&profile))
}

fn chrome_profile(app: &AppHandle, profile_id: &str) -> Result<String, AppError> {
    let settings = storage::load_settings(&app_paths(app)?.config_dir)?;
    firefox::scan_firefox(&settings)
        .profiles
        .into_iter()
        .find(|profile| profile.id == profile_id && profile.error.is_none())
        .map(|profile| profile.path)
        .ok_or_else(|| {
            AppError::new(
                "profile_unavailable",
                "Rescan Firefox and select an available profile.",
                None,
            )
        })
}

#[tauri::command]
fn set_app_chrome(app: AppHandle, profile_id: String, enabled: bool) -> Result<(), AppError> {
    let profile = chrome_profile(&app, &profile_id)?;
    app_chrome::set_enabled(std::path::Path::new(&profile), enabled)
}

#[tauri::command]
fn update_app_chrome(app: AppHandle, profile_id: String) -> Result<(), AppError> {
    let profile = chrome_profile(&app, &profile_id)?;
    app_chrome::update(std::path::Path::new(&profile))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            app_chrome_status,
            update_app_chrome,
            set_app_chrome,
            get_settings,
            save_settings,
            scan_firefox,
            list_managed_shortcuts,
            create_shortcut,
            update_shortcut,
            refresh_shortcut_icon,
            delete_shortcut,
            launch_shortcut,
            open_shortcut_location,
            open_start_menu_folder,
            open_extension_page
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
