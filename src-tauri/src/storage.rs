use crate::{
    errors::AppError,
    models::{AppSettings, ShortcutRecord},
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
use uuid::Uuid;

const SETTINGS_SCHEMA_VERSION: u32 = 1;
const SHORTCUTS_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug)]
pub struct AppPaths {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManagedShortcutStore<T> {
    schema_version: u32,
    shortcuts: T,
}

pub fn load_settings(config_dir: &Path) -> Result<AppSettings, AppError> {
    let path = config_dir.join("settings.json");
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(AppSettings::default());
        }
        Err(error) => {
            return Err(AppError::io(
                "Application settings could not be read.",
                error,
            ));
        }
    };

    let settings: AppSettings = serde_json::from_str(&contents).map_err(|error| {
        AppError::new(
            "invalid_settings",
            "Application settings are invalid. Reset or repair the settings file.",
            Some(error.to_string()),
        )
    })?;
    validate_settings(&settings)?;
    Ok(settings)
}

pub fn save_settings(config_dir: &Path, settings: &AppSettings) -> Result<(), AppError> {
    validate_settings(settings)?;
    let mut settings = settings.clone();
    settings.schema_version = SETTINGS_SCHEMA_VERSION;
    let contents = serde_json::to_vec_pretty(&settings).map_err(|error| {
        AppError::new(
            "settings_save_failed",
            "Application settings could not be encoded.",
            Some(error.to_string()),
        )
    })?;
    write_atomic(&config_dir.join("settings.json"), &contents)
}

pub fn load_shortcuts(data_dir: &Path) -> Result<Vec<ShortcutRecord>, AppError> {
    let path = data_dir.join("managed-shortcuts.json");
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(AppError::io(
                "Managed shortcut metadata could not be read.",
                error,
            ));
        }
    };

    let store: ManagedShortcutStore<Vec<ShortcutRecord>> = serde_json::from_str(&contents)
        .map_err(|error| {
            AppError::new(
                "invalid_shortcut_metadata",
                "Managed shortcut metadata is invalid and could not be loaded.",
                Some(error.to_string()),
            )
        })?;
    if store.schema_version > SHORTCUTS_SCHEMA_VERSION {
        return Err(AppError::new(
            "unsupported_shortcut_metadata",
            "The managed shortcut data was created by a newer application version.",
            None,
        ));
    }

    Ok(store.shortcuts)
}

pub fn save_shortcuts(data_dir: &Path, shortcuts: &[ShortcutRecord]) -> Result<(), AppError> {
    let store = ManagedShortcutStore {
        schema_version: SHORTCUTS_SCHEMA_VERSION,
        shortcuts,
    };
    let contents = serde_json::to_vec_pretty(&store).map_err(|error| {
        AppError::new(
            "shortcut_metadata_save_failed",
            "Managed shortcut metadata could not be encoded.",
            Some(error.to_string()),
        )
    })?;
    write_atomic(&data_dir.join("managed-shortcuts.json"), &contents)
}

fn validate_settings(settings: &AppSettings) -> Result<(), AppError> {
    if settings.schema_version > SETTINGS_SCHEMA_VERSION {
        return Err(AppError::new(
            "unsupported_settings",
            "The settings file was created by a newer application version.",
            None,
        ));
    }
    if settings.custom_firefox_paths.len() > 20 {
        return Err(AppError::new(
            "invalid_settings",
            "At most 20 custom Firefox executable paths can be configured.",
            None,
        ));
    }
    if settings
        .custom_firefox_paths
        .iter()
        .any(|path| path.trim().is_empty() || path.len() > 2_048)
    {
        return Err(AppError::new(
            "invalid_settings",
            "Custom Firefox executable paths must not be empty or exceed 2,048 characters.",
            None,
        ));
    }
    Ok(())
}

fn write_atomic(path: &Path, contents: &[u8]) -> Result<(), AppError> {
    let parent = path.parent().ok_or_else(|| {
        AppError::new(
            "filesystem_error",
            "The application data directory is unavailable.",
            Some(path.display().to_string()),
        )
    })?;
    fs::create_dir_all(parent).map_err(|error| {
        AppError::io(
            "The application data directory could not be created.",
            error,
        )
    })?;

    let temp_path = parent.join(format!(".{}.{}.tmp", file_stem(path), Uuid::new_v4()));
    fs::write(&temp_path, contents)
        .map_err(|error| AppError::io("Application data could not be written.", error))?;

    if let Err(error) = fs::rename(&temp_path, path) {
        let _ = fs::remove_file(&temp_path);
        return Err(AppError::io(
            "Application data could not be saved safely.",
            error,
        ));
    }

    Ok(())
}

fn file_stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("data")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_settings_use_safe_defaults() {
        let temp = std::env::temp_dir().join(format!("fsm-settings-{}", Uuid::new_v4()));
        let settings = load_settings(&temp).unwrap();
        assert_eq!(settings.schema_version, 1);
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn ignores_legacy_custom_start_menu_folder_setting() {
        let temp = std::env::temp_dir().join(format!("fsm-settings-{}", Uuid::new_v4()));
        fs::create_dir_all(&temp).unwrap();
        fs::write(
            temp.join("settings.json"),
            r#"{"schemaVersion":1,"theme":"system","defaultInstallationId":null,"defaultProfileId":null,"startMenuSubfolder":"Custom Folder","autoScan":true,"customFirefoxPaths":[]}"#,
        )
        .unwrap();

        assert!(load_settings(&temp).unwrap().auto_scan);
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn shortcut_store_round_trips_without_browser_secrets() {
        let temp = std::env::temp_dir().join(format!("fsm-shortcuts-{}", Uuid::new_v4()));
        fs::create_dir_all(&temp).unwrap();
        let record = ShortcutRecord {
            id: "id".to_string(),
            name: "Teams (Work)".to_string(),
            website_name: "Teams".to_string(),
            target_url: "https://teams.microsoft.com/".to_string(),
            installation_id: "installation".to_string(),
            profile_id: "profile".to_string(),
            profile_path: "C:\\Firefox\\Profile".to_string(),
            profile_name: "Default".to_string(),
            container_id: Some("firefox-container-1".to_string()),
            container_name: Some("Work".to_string()),
            container_user_context_id: Some(1),
            taskbar_tab_id: Some("99314857-fcfa-4f1a-8fb2-088f24e95a3f".to_string()),
            open_as_app: true,
            protocol_url: "ext+container:".to_string(),
            shortcut_path: "Firefox Shortcuts\\Teams (Work).lnk".to_string(),
            icon_path: "C:\\Icons\\teams.ico".to_string(),
            icon_background_color: Some("#ffffff".to_string()),
            icon_used_fallback: true,
            icon_warning: Some("A generated icon is being used.".to_string()),
            created_at: 1,
        };

        let mut no_container_record = record.clone();
        no_container_record.id = "no-container".to_string();
        no_container_record.container_id = None;
        no_container_record.container_name = None;
        no_container_record.container_user_context_id = None;
        no_container_record.open_as_app = false;
        no_container_record.protocol_url = no_container_record.target_url.clone();
        save_shortcuts(&temp, &[record.clone(), no_container_record]).unwrap();
        let loaded = load_shortcuts(&temp).unwrap();
        assert_eq!(loaded[0].id, record.id);
        assert_eq!(
            loaded[0].container_id.as_deref(),
            Some("firefox-container-1")
        );
        assert_eq!(loaded[0].container_user_context_id, Some(1));
        assert_eq!(
            loaded[0].taskbar_tab_id.as_deref(),
            Some("99314857-fcfa-4f1a-8fb2-088f24e95a3f")
        );
        assert!(loaded[0].open_as_app);
        assert!(loaded[1].container_id.is_none());
        assert!(loaded[1].container_name.is_none());
        assert!(!loaded[1].open_as_app);
        assert!(loaded[0].icon_used_fallback);
        assert_eq!(loaded[0].icon_background_color.as_deref(), Some("#ffffff"));
        assert_eq!(
            loaded[0].icon_warning.as_deref(),
            Some("A generated icon is being used.")
        );

        let mut legacy_record = serde_json::to_value(&record).unwrap();
        let legacy_object = legacy_record.as_object_mut().unwrap();
        legacy_object.remove("iconUsedFallback");
        legacy_object.remove("iconBackgroundColor");
        legacy_object.remove("iconWarning");
        legacy_object.remove("containerUserContextId");
        legacy_object.remove("taskbarTabId");
        legacy_object.remove("openAsApp");
        let legacy_store = serde_json::json!({
            "schemaVersion": SHORTCUTS_SCHEMA_VERSION,
            "shortcuts": [legacy_record],
        });
        fs::write(
            temp.join("managed-shortcuts.json"),
            serde_json::to_vec(&legacy_store).unwrap(),
        )
        .unwrap();
        let legacy_loaded = load_shortcuts(&temp).unwrap();
        assert!(legacy_loaded[0].container_user_context_id.is_none());
        assert!(legacy_loaded[0].taskbar_tab_id.is_none());
        assert!(!legacy_loaded[0].open_as_app);
        assert!(!legacy_loaded[0].icon_used_fallback);
        assert!(legacy_loaded[0].icon_background_color.is_none());
        assert!(legacy_loaded[0].icon_warning.is_none());
        let _ = fs::remove_dir_all(temp);
    }
}
