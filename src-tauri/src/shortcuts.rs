use crate::{
    errors::AppError,
    firefox,
    icons::{self, IconOutcome},
    models::{
        AppSettings, CreateShortcutRequest, ExtensionDetectionState, FirefoxContainer,
        FirefoxInstallation, FirefoxProfile, ShortcutRecord, ShortcutStatus, ShortcutView,
    },
    protocol,
    storage::{self, AppPaths},
    taskbar_tabs,
    windows::shortcuts::{ShortcutTarget, create_shortcut as write_windows_shortcut},
};
use std::{
    fs,
    path::{Component, Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;

const START_MENU_SHORTCUT_FOLDER: &str = "Firefox Shortcuts";

pub fn create_managed_shortcut(
    paths: &AppPaths,
    request: CreateShortcutRequest,
) -> Result<ShortcutView, AppError> {
    save_managed_shortcut(paths, request, None)
}

pub fn update_managed_shortcut(
    paths: &AppPaths,
    shortcut_id: &str,
    request: CreateShortcutRequest,
) -> Result<ShortcutView, AppError> {
    save_managed_shortcut(paths, request, Some(shortcut_id))
}

fn save_managed_shortcut(
    paths: &AppPaths,
    request: CreateShortcutRequest,
    shortcut_id: Option<&str>,
) -> Result<ShortcutView, AppError> {
    let mut records = storage::load_shortcuts(&paths.data_dir)?;
    let existing_index = shortcut_id
        .map(|id| shortcut_index(&records, id))
        .transpose()?;
    let existing_record = existing_index.map(|index| &records[index]);

    let settings = storage::load_settings(&paths.config_dir)?;
    let scan = firefox::scan_firefox(&settings);
    let installation = find_installation(&scan.installations, &request.installation_id)?;
    let profile = find_profile(&scan.profiles, &request.profile_id)?;
    let container = match request.container_id.as_deref() {
        Some(container_id) => Some(
            profile
                .containers
                .iter()
                .find(|container| container.id == container_id)
                .ok_or_else(|| {
                    AppError::new(
                        "container_not_found",
                        "The selected container was not found in this Firefox profile. Rescan Firefox and choose an existing container.",
                        Some(container_id.to_string()),
                    )
                })?,
        ),
        None => None,
    };
    if container.is_some() && !request.open_as_app {
        ensure_external_handler(profile)?;
    }

    let website_url = protocol::validate_website_url(&request.target_url)?;
    let website_name = validate_display_name(&request.website_name)?;
    let shortcut_name = validate_shortcut_name(&request.shortcut_name)?;
    let start_menu = start_menu_programs_dir()?;
    let destination = shortcut_destination(&start_menu, &shortcut_name);
    let previous_path = existing_record
        .map(|record| resolve_managed_path(&start_menu, &record.shortcut_path))
        .transpose()?;
    let replace_existing = previous_path
        .as_ref()
        .is_some_and(|path| same_managed_destination(path, &destination));
    let relative_path = destination.strip_prefix(&start_menu).map_err(|error| {
        AppError::new(
            "invalid_destination",
            "The shortcut destination must remain inside the current user's Start menu.",
            Some(error.to_string()),
        )
    })?;
    let metadata_conflict = records.iter().enumerate().any(|(index, record)| {
        Some(index) != existing_index
            && same_managed_destination(Path::new(&record.shortcut_path), relative_path)
    });
    if (destination.exists() && !replace_existing) || metadata_conflict {
        return Err(AppError::new(
            "shortcut_exists",
            "A shortcut with this name already exists. Choose another name.",
            Some(destination.display().to_string()),
        ));
    }

    let protocol_url = shortcut_launch_url(
        request.open_as_app,
        container.map(|container| container.id.as_str()),
        container.map(|container| container.name.as_str()),
        &website_url,
    )?;
    let icon_outcome = icons::retrieve_icon(
        &website_url,
        &website_name,
        &paths.cache_dir.join("icons"),
        request.icon_background_color.as_deref(),
        false,
    )
    .map_err(|error| {
        AppError::new(
            "icon_creation_failed",
            "A website icon could not be created for this shortcut.",
            Some(error),
        )
    })?;
    let mut record = build_record(
        &request,
        installation,
        profile,
        container,
        website_name,
        website_url,
        protocol_url,
        relative_path,
        &icon_outcome,
        existing_record,
    )?;
    let taskbar_registration = if record.open_as_app {
        Some(taskbar_tabs::register_taskbar_tab(
            Path::new(&record.profile_path),
            &record.id,
            record.taskbar_tab_id.as_deref(),
            &record.website_name,
            &record.target_url,
            firefox_user_context_id(&record)?,
        )?)
    } else {
        None
    };
    if let Some(registration) = taskbar_registration.as_ref() {
        record.taskbar_tab_id = Some(registration.id().to_string());
    }

    let preview_data_url = icon_outcome.preview_data_url;

    let persist = || -> Result<(), AppError> {
        if let Some(index) = existing_index {
            let original = std::mem::replace(&mut records[index], record.clone());
            storage::save_shortcuts(&paths.data_dir, &records)?;

            if let Err(error) =
                write_record_shortcut(&record, &destination, installation, replace_existing)
            {
                records[index] = original;
                return Err(match storage::save_shortcuts(&paths.data_dir, &records) {
                    Ok(()) => error,
                    Err(rollback_error) => AppError::new(
                        "shortcut_update_rollback_failed",
                        "The shortcut could not be updated, and its original metadata could not be restored.",
                        Some(format!(
                            "Shortcut write error: {error}; metadata rollback error: {rollback_error}"
                        )),
                    ),
                });
            }

            if let Some(previous_path) = previous_path.filter(|_| !replace_existing)
                && let Err(error) = remove_if_exists(&previous_path)
            {
                records[index] = original;
                storage::save_shortcuts(&paths.data_dir, &records).map_err(|rollback_error| {
                    AppError::new(
                        "shortcut_update_rollback_failed",
                        "The updated shortcut was saved, but the previous shortcut could not be removed and its original metadata could not be restored.",
                        Some(format!(
                            "Previous shortcut removal error: {error}; metadata rollback error: {rollback_error}; new shortcut: {}",
                            destination.display()
                        )),
                    )
                })?;
                remove_if_exists(&destination).map_err(|cleanup_error| {
                    AppError::new(
                        "shortcut_update_cleanup_failed",
                        "The edit was rolled back, but the new Windows shortcut could not be removed.",
                        Some(format!(
                            "Previous shortcut removal error: {error}; new shortcut cleanup error: {cleanup_error}"
                        )),
                    )
                })?;
                return Err(AppError::io(
                    "The previous Windows shortcut could not be removed. The edit was rolled back.",
                    error,
                ));
            }
        } else {
            write_record_shortcut(&record, &destination, installation, false)?;
            records.push(record.clone());
            if let Err(error) = storage::save_shortcuts(&paths.data_dir, &records) {
                remove_if_exists(&destination).map_err(|cleanup_error| {
                    AppError::new(
                        "shortcut_create_rollback_failed",
                        "Shortcut metadata could not be saved, and the new Windows shortcut could not be removed.",
                        Some(format!(
                            "Metadata save error: {error}; shortcut cleanup error: {cleanup_error}"
                        )),
                    )
                })?;
                return Err(error);
            }
        }
        Ok(())
    };
    persist().map_err(|error| rollback_taskbar_registration(error, taskbar_registration))?;

    let status = shortcut_status(&record, &scan);
    Ok(view_from_record(record, status, Some(preview_data_url)))
}

fn rollback_taskbar_registration(
    operation_error: AppError,
    registration: Option<taskbar_tabs::TaskbarTabRegistration>,
) -> AppError {
    let Some(registration) = registration else {
        return operation_error;
    };

    match registration.rollback() {
        Ok(()) => operation_error,
        Err(rollback_error) => AppError::new(
            "taskbar_tabs_rollback_failed",
            "The shortcut operation failed, and Firefox's app registration could not be restored.",
            Some(format!(
                "Operation error: {operation_error}; registration rollback error: {rollback_error}"
            )),
        ),
    }
}

pub fn refresh_managed_shortcut_icon(
    paths: &AppPaths,
    shortcut_id: &str,
) -> Result<ShortcutView, AppError> {
    let mut records = storage::load_shortcuts(&paths.data_dir)?;
    let index = shortcut_index(&records, shortcut_id)?;
    let settings = storage::load_settings(&paths.config_dir)?;
    let scan = firefox::scan_firefox(&settings);
    let installation = find_installation(&scan.installations, &records[index].installation_id)?;
    let record_path =
        resolve_managed_path(&start_menu_programs_dir()?, &records[index].shortcut_path)?;
    if !record_path.is_file() {
        return Err(AppError::new(
            "shortcut_missing",
            "The Windows shortcut no longer exists, so its icon cannot be refreshed.",
            Some(record_path.display().to_string()),
        ));
    }

    let mut record = records[index].clone();
    record.protocol_url = shortcut_launch_url(
        record.open_as_app,
        record.container_id.as_deref(),
        record.container_name.as_deref(),
        &record.target_url,
    )?;
    let icon = icons::retrieve_icon(
        &record.target_url,
        &record.website_name,
        &paths.cache_dir.join("icons"),
        record.icon_background_color.as_deref(),
        true,
    )
    .map_err(|error| {
        AppError::new(
            "icon_refresh_failed",
            "The website icon could not be refreshed.",
            Some(error),
        )
    })?;
    record.icon_path = icon.icon_path;
    record.icon_used_fallback = icon.used_fallback;
    record.icon_warning = icon.warning;

    write_record_shortcut(&record, &record_path, installation, true)?;
    records[index] = record.clone();
    storage::save_shortcuts(&paths.data_dir, &records)?;

    let status = shortcut_status(&record, &scan);
    Ok(view_from_record(
        record,
        status,
        Some(icon.preview_data_url),
    ))
}

pub fn list_managed_shortcuts(
    paths: &AppPaths,
    settings: &AppSettings,
) -> Result<Vec<ShortcutView>, AppError> {
    let mut records = storage::load_shortcuts(&paths.data_dir)?;
    let scan = firefox::scan_firefox(settings);
    repair_protocol_urls(paths, &mut records, &scan)?;

    Ok(records
        .into_iter()
        .map(|record| {
            let status = shortcut_status(&record, &scan);
            let preview = icons::preview_from_icon(Path::new(&record.icon_path));
            view_from_record(record, status, preview)
        })
        .collect())
}

fn repair_protocol_urls(
    paths: &AppPaths,
    records: &mut [ShortcutRecord],
    scan: &crate::models::ScanResult,
) -> Result<(), AppError> {
    let mut changed = false;
    for record in records.iter_mut() {
        let protocol_url = shortcut_launch_url(
            record.open_as_app,
            record.container_id.as_deref(),
            record.container_name.as_deref(),
            &record.target_url,
        )?;
        if protocol_url == record.protocol_url {
            continue;
        }
        let Some(installation) = scan
            .installations
            .iter()
            .find(|installation| installation.id == record.installation_id)
        else {
            continue;
        };
        let start_menu = start_menu_programs_dir()?;
        let shortcut_path = resolve_managed_path(&start_menu, &record.shortcut_path)?;
        if !shortcut_path.is_file() {
            continue;
        }

        let mut updated_record = record.clone();
        updated_record.protocol_url = protocol_url;
        write_record_shortcut(&updated_record, &shortcut_path, installation, true)?;
        *record = updated_record;
        changed = true;
    }

    if changed {
        storage::save_shortcuts(&paths.data_dir, records)?;
    }
    Ok(())
}

pub fn delete_managed_shortcut(paths: &AppPaths, shortcut_id: &str) -> Result<(), AppError> {
    let mut records = storage::load_shortcuts(&paths.data_dir)?;
    let index = shortcut_index(&records, shortcut_id)?;
    let record = records.remove(index);
    let start_menu = start_menu_programs_dir()?;
    let shortcut_path = resolve_managed_path(&start_menu, &record.shortcut_path)?;

    remove_if_exists(&shortcut_path).map_err(|error| {
        AppError::io("The managed Windows shortcut could not be removed.", error)
    })?;
    storage::save_shortcuts(&paths.data_dir, &records)
}

fn shortcut_index(records: &[ShortcutRecord], id: &str) -> Result<usize, AppError> {
    records
        .iter()
        .position(|record| record.id == id)
        .ok_or_else(|| {
            AppError::new(
                "shortcut_not_found",
                "The managed shortcut could not be found.",
                Some(id.to_string()),
            )
        })
}

fn remove_if_exists(path: &Path) -> std::io::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}

pub fn launch_managed_shortcut(paths: &AppPaths, shortcut_id: &str) -> Result<(), AppError> {
    let records = storage::load_shortcuts(&paths.data_dir)?;
    let record = &records[shortcut_index(&records, shortcut_id)?];
    let start_menu = start_menu_programs_dir()?;
    let shortcut_path = resolve_managed_path(&start_menu, &record.shortcut_path)?;
    if !shortcut_path.is_file() {
        return Err(AppError::new(
            "shortcut_missing",
            "The Windows shortcut no longer exists. Recreate it from the Shortcuts page.",
            Some(shortcut_path.display().to_string()),
        ));
    }

    Command::new("explorer.exe")
        .arg(&shortcut_path)
        .spawn()
        .map(|_| ())
        .map_err(|error| {
            AppError::new(
                "shortcut_launch_failed",
                "Windows could not launch this shortcut.",
                Some(error.to_string()),
            )
        })
}

pub fn open_shortcut_location(paths: &AppPaths, shortcut_id: &str) -> Result<(), AppError> {
    let records = storage::load_shortcuts(&paths.data_dir)?;
    let record = &records[shortcut_index(&records, shortcut_id)?];
    let root = start_menu_programs_dir()?;
    let shortcut_path = resolve_managed_path(&root, &record.shortcut_path)?;
    let folder = shortcut_path.parent().ok_or_else(|| {
        AppError::new(
            "invalid_destination",
            "The shortcut folder is not valid.",
            None,
        )
    })?;
    open_folder(folder)
}

pub fn open_start_menu_folder() -> Result<(), AppError> {
    open_folder(&start_menu_programs_dir()?)
}

pub fn open_extension_page(
    paths: &AppPaths,
    installation_id: &str,
    profile_id: &str,
    url: &str,
) -> Result<(), AppError> {
    let settings = storage::load_settings(&paths.config_dir)?;
    let scan = firefox::scan_firefox(&settings);
    let installation = find_installation(&scan.installations, installation_id)?;
    let profile = find_profile(&scan.profiles, profile_id)?;
    let target_url = protocol::validate_website_url(url)?;
    launch_in_profile(installation, profile, &target_url)
}

pub fn open_folder(path: &Path) -> Result<(), AppError> {
    Command::new("explorer.exe")
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|error| {
            AppError::new(
                "folder_open_failed",
                "Windows could not open this folder.",
                Some(error.to_string()),
            )
        })
}

fn write_record_shortcut(
    record: &ShortcutRecord,
    destination: &Path,
    installation: &FirefoxInstallation,
    replace_existing: bool,
) -> Result<(), AppError> {
    let executable = Path::new(&installation.executable);
    let working_directory = executable.parent().unwrap_or_else(|| Path::new(""));
    let icon_path = Path::new(&record.icon_path);
    let arguments = build_firefox_arguments_for_record(record)?;
    let description = match (
        record.open_as_app,
        &record.container_id,
        &record.container_name,
    ) {
        (true, Some(_), Some(container_name)) => format!(
            "Open {} as a Firefox app in the Firefox container {}",
            record.website_name, container_name
        ),
        (true, None, None) => format!("Open {} as a Firefox app", record.website_name),
        (false, Some(_), Some(container_name)) => format!(
            "Open {} in the Firefox container {}",
            record.website_name, container_name
        ),
        (false, None, None) => format!("Open {} directly in Firefox", record.website_name),
        _ => {
            return Err(AppError::new(
                "invalid_shortcut_metadata",
                "A managed shortcut has incomplete container metadata.",
                Some(record.id.clone()),
            ));
        }
    };

    write_windows_shortcut(
        destination,
        &ShortcutTarget {
            executable,
            arguments: &arguments,
            working_directory,
            icon_path,
            description: &description,
        },
        replace_existing,
    )
}

fn shortcut_launch_url(
    open_as_app: bool,
    container_id: Option<&str>,
    container_name: Option<&str>,
    website_url: &str,
) -> Result<String, AppError> {
    if open_as_app {
        return protocol::validate_website_url(website_url);
    }

    match (container_id, container_name) {
        (Some(container_id), Some(container_name)) => {
            protocol::generate_container_url(container_id, container_name, website_url)
        }
        (None, None) => protocol::validate_website_url(website_url),
        _ => Err(AppError::new(
            "invalid_shortcut_metadata",
            "A managed shortcut has incomplete container metadata.",
            None,
        )),
    }
}

fn build_record(
    request: &CreateShortcutRequest,
    installation: &FirefoxInstallation,
    profile: &FirefoxProfile,
    container: Option<&FirefoxContainer>,
    website_name: String,
    website_url: String,
    protocol_url: String,
    relative_path: &Path,
    icon: &IconOutcome,
    existing_record: Option<&ShortcutRecord>,
) -> Result<ShortcutRecord, AppError> {
    let (id, created_at, taskbar_tab_id) = match existing_record {
        Some(record) => (
            record.id.clone(),
            record.created_at,
            record.taskbar_tab_id.clone(),
        ),
        None => {
            let created_at = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|error| {
                    AppError::new(
                        "clock_error",
                        "The shortcut creation time could not be recorded.",
                        Some(error.to_string()),
                    )
                })?
                .as_secs();
            (Uuid::new_v4().to_string(), created_at, None)
        }
    };

    Ok(ShortcutRecord {
        id,
        name: request.shortcut_name.trim().to_string(),
        website_name,
        target_url: website_url,
        installation_id: installation.id.clone(),
        profile_id: profile.id.clone(),
        profile_path: profile.path.clone(),
        profile_name: profile.name.clone(),
        container_id: container.map(|container| container.id.clone()),
        container_name: container.map(|container| container.name.clone()),
        container_user_context_id: container.map(|container| container.user_context_id),
        taskbar_tab_id,
        open_as_app: request.open_as_app,
        protocol_url,
        shortcut_path: relative_path.to_string_lossy().into_owned(),
        icon_path: icon.icon_path.clone(),
        icon_background_color: request.icon_background_color.clone(),
        icon_used_fallback: icon.used_fallback,
        icon_warning: icon.warning.clone(),
        created_at,
    })
}

fn firefox_user_context_id(record: &ShortcutRecord) -> Result<u32, AppError> {
    match (
        record.container_id.as_deref(),
        record.container_name.as_deref(),
        record.container_user_context_id,
    ) {
        (Some(_), Some(_), Some(user_context_id)) => Ok(user_context_id),
        (None, None, None) => Ok(0),
        _ => Err(AppError::new(
            "invalid_shortcut_metadata",
            "A managed app shortcut has incomplete container metadata.",
            Some(record.id.clone()),
        )),
    }
}

fn shortcut_status(record: &ShortcutRecord, scan: &crate::models::ScanResult) -> ShortcutStatus {
    let start_menu = match start_menu_programs_dir() {
        Ok(directory) => directory,
        Err(_) => return ShortcutStatus::DetectionFailed,
    };
    let shortcut_path = match resolve_managed_path(&start_menu, &record.shortcut_path) {
        Ok(path) => path,
        Err(_) => return ShortcutStatus::DetectionFailed,
    };
    if !shortcut_path.is_file() {
        return ShortcutStatus::MissingShortcut;
    }

    if !scan
        .installations
        .iter()
        .any(|installation| installation.id == record.installation_id)
    {
        return ShortcutStatus::MissingFirefox;
    }

    let Some(profile) = scan
        .profiles
        .iter()
        .find(|profile| profile.id == record.profile_id)
    else {
        return ShortcutStatus::MissingProfile;
    };
    let container_id = match (&record.container_id, &record.container_name) {
        (Some(container_id), Some(_)) => Some(container_id),
        (None, None) => None,
        _ => return ShortcutStatus::DetectionFailed,
    };
    if let Some(container_id) = container_id {
        if !profile
            .containers
            .iter()
            .any(|container| &container.id == container_id)
        {
            return ShortcutStatus::MissingContainer;
        }
    }
    if record.open_as_app {
        let taskbar_tab_id = record.taskbar_tab_id.as_deref().unwrap_or(&record.id);
        let Ok(user_context_id) = firefox_user_context_id(record) else {
            return ShortcutStatus::DetectionFailed;
        };
        return match taskbar_tabs::matches_taskbar_tab(
            Path::new(&record.profile_path),
            taskbar_tab_id,
            &record.target_url,
            user_context_id,
        ) {
            Ok(true) => ShortcutStatus::Ready,
            Ok(false) => ShortcutStatus::MissingTaskbarTab,
            Err(_) => ShortcutStatus::DetectionFailed,
        };
    }
    if container_id.is_none() {
        return ShortcutStatus::Ready;
    }

    match profile
        .extensions
        .iter()
        .find(|extension| extension.id == "{f069aec0-43c5-4bbf-b6b4-df95c4326b98}")
        .map(|extension| &extension.status)
    {
        Some(ExtensionDetectionState::InstalledEnabled)
        | Some(ExtensionDetectionState::InstalledUnknown) => ShortcutStatus::Ready,
        Some(ExtensionDetectionState::NotInstalled)
        | Some(ExtensionDetectionState::InstalledDisabled) => ShortcutStatus::MissingHandler,
        Some(ExtensionDetectionState::DetectionFailed) | None => ShortcutStatus::DetectionFailed,
    }
}

fn view_from_record(
    record: ShortcutRecord,
    status: ShortcutStatus,
    icon_preview_data_url: Option<String>,
) -> ShortcutView {
    ShortcutView {
        id: record.id,
        name: record.name,
        website_name: record.website_name,
        target_url: record.target_url,
        installation_id: record.installation_id,
        profile_id: record.profile_id,
        profile_name: record.profile_name,
        container_id: record.container_id,
        container_name: record.container_name,
        open_as_app: record.open_as_app,
        protocol_url: record.protocol_url,
        shortcut_path: record.shortcut_path,
        icon_path: record.icon_path,
        icon_preview_data_url,
        icon_background_color: record.icon_background_color,
        icon_used_fallback: record.icon_used_fallback,
        icon_warning: record.icon_warning,
        status,
        created_at: record.created_at,
    }
}

fn find_installation<'a>(
    installations: &'a [FirefoxInstallation],
    installation_id: &str,
) -> Result<&'a FirefoxInstallation, AppError> {
    installations
        .iter()
        .find(|installation| installation.id == installation_id)
        .ok_or_else(|| {
            AppError::new(
                "firefox_not_found",
                "The selected Firefox installation could not be found. Rescan Firefox and try again.",
                Some(installation_id.to_string()),
            )
        })
}

fn find_profile<'a>(
    profiles: &'a [FirefoxProfile],
    profile_id: &str,
) -> Result<&'a FirefoxProfile, AppError> {
    profiles
        .iter()
        .find(|profile| profile.id == profile_id)
        .ok_or_else(|| {
            AppError::new(
                "profile_not_found",
                "The selected Firefox profile could not be found. Rescan Firefox and try again.",
                Some(profile_id.to_string()),
            )
        })
        .and_then(|profile| {
            if profile.error.is_some() {
                Err(AppError::new(
                    "profile_unavailable",
                    "The selected Firefox profile is unavailable. Check its path or rescan Firefox.",
                    profile.error.clone(),
                ))
            } else {
                Ok(profile)
            }
        })
}

fn ensure_external_handler(profile: &FirefoxProfile) -> Result<(), AppError> {
    let extension = profile
        .extensions
        .iter()
        .find(|extension| extension.id == "{f069aec0-43c5-4bbf-b6b4-df95c4326b98}");

    match extension.map(|extension| &extension.status) {
        Some(ExtensionDetectionState::InstalledEnabled)
        | Some(ExtensionDetectionState::InstalledUnknown) => Ok(()),
        Some(ExtensionDetectionState::InstalledDisabled) => Err(AppError::new(
            "extension_disabled",
            "Enable “Open external links in a container” in the selected Firefox profile, then rescan.",
            None,
        )),
        Some(ExtensionDetectionState::NotInstalled) => Err(AppError::new(
            "extension_missing",
            "Install “Open external links in a container” in the selected Firefox profile, then rescan.",
            None,
        )),
        Some(ExtensionDetectionState::DetectionFailed) | None => Err(AppError::new(
            "extension_detection_failed",
            "The external-container extension could not be verified for this profile. Close Firefox and rescan.",
            None,
        )),
    }
}

fn launch_in_profile(
    installation: &FirefoxInstallation,
    profile: &FirefoxProfile,
    url: &str,
) -> Result<(), AppError> {
    Command::new(&installation.executable)
        .args(firefox_tab_arguments(&profile.path, url))
        .spawn()
        .map(|_| ())
        .map_err(|error| {
            AppError::new(
                "firefox_launch_failed",
                "Firefox could not be started with the selected profile. If this profile is already open, close it and try again.",
                Some(error.to_string()),
            )
        })
}

fn firefox_tab_arguments<'a>(profile_path: &'a str, url: &'a str) -> [&'a str; 5] {
    ["--profile", profile_path, "--no-remote", "--new-tab", url]
}

fn build_firefox_arguments(profile_path: &str, protocol_url: &str) -> String {
    quote_windows_arguments(&firefox_tab_arguments(profile_path, protocol_url))
}

fn build_firefox_arguments_for_record(record: &ShortcutRecord) -> Result<String, AppError> {
    if !record.open_as_app {
        return Ok(build_firefox_arguments(
            &record.profile_path,
            &record.protocol_url,
        ));
    }

    let container_user_context_id = firefox_user_context_id(record)?;

    Ok(build_firefox_app_arguments(
        record.taskbar_tab_id.as_deref().unwrap_or(&record.id),
        &record.target_url,
        &record.profile_path,
        container_user_context_id,
    ))
}

fn build_firefox_app_arguments(
    shortcut_id: &str,
    target_url: &str,
    profile_path: &str,
    container_user_context_id: u32,
) -> String {
    quote_windows_arguments(&[
        "-taskbar-tab",
        shortcut_id,
        "-new-window",
        target_url,
        "-profile",
        profile_path,
        "-container",
        &container_user_context_id.to_string(),
    ])
}

fn quote_windows_arguments(arguments: &[&str]) -> String {
    arguments
        .iter()
        .map(|argument| quote_windows_argument(argument))
        .collect::<Vec<_>>()
        .join(" ")
}

fn quote_windows_argument(argument: &str) -> String {
    let mut quoted = String::from("\"");
    let mut backslashes = 0;

    for character in argument.chars() {
        match character {
            '\\' => backslashes += 1,
            '"' => {
                quoted.push_str(&"\\".repeat(backslashes * 2 + 1));
                quoted.push('"');
                backslashes = 0;
            }
            _ => {
                quoted.push_str(&"\\".repeat(backslashes));
                quoted.push(character);
                backslashes = 0;
            }
        }
    }

    quoted.push_str(&"\\".repeat(backslashes * 2));
    quoted.push('"');
    quoted
}

fn shortcut_destination(programs_dir: &Path, name: &str) -> PathBuf {
    programs_dir
        .join(START_MENU_SHORTCUT_FOLDER)
        .join(format!("{name}.lnk"))
}

fn same_managed_destination(left: &Path, right: &Path) -> bool {
    #[cfg(windows)]
    {
        left.to_string_lossy().to_lowercase() == right.to_string_lossy().to_lowercase()
    }
    #[cfg(not(windows))]
    {
        left == right
    }
}

fn validate_shortcut_name(name: &str) -> Result<String, AppError> {
    let name = name.trim();
    validate_windows_segment(name)?;
    if name.len() > 120 {
        return Err(AppError::new(
            "invalid_shortcut_name",
            "Shortcut names must be 120 characters or fewer.",
            None,
        ));
    }
    Ok(name.to_string())
}

fn validate_display_name(name: &str) -> Result<String, AppError> {
    let name = name.trim();
    if name.is_empty() || name.len() > 120 || name.chars().any(char::is_control) {
        return Err(AppError::new(
            "invalid_display_name",
            "Enter a website name between 1 and 120 characters.",
            None,
        ));
    }
    Ok(name.to_string())
}

fn validate_windows_segment(segment: &str) -> Result<(), AppError> {
    let reserved_name = segment
        .split('.')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_uppercase();
    let is_reserved = matches!(reserved_name.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || reserved_name
            .strip_prefix("COM")
            .or_else(|| reserved_name.strip_prefix("LPT"))
            .is_some_and(|number| number.len() == 1 && matches!(number.as_bytes()[0], b'1'..=b'9'));

    if segment.is_empty()
        || segment == "."
        || segment == ".."
        || segment.ends_with(['.', ' '])
        || segment
            .chars()
            .any(|character| character.is_control() || "<>:\"/\\|?*".contains(character))
        || is_reserved
    {
        return Err(AppError::new(
            "invalid_destination",
            "Use a Windows-safe shortcut name and Start menu folder.",
            Some(segment.to_string()),
        ));
    }
    Ok(())
}

fn resolve_managed_path(root: &Path, relative_path: &str) -> Result<PathBuf, AppError> {
    let relative = Path::new(relative_path);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        || relative
            .extension()
            .is_none_or(|extension| extension != "lnk")
    {
        return Err(AppError::new(
            "invalid_shortcut_metadata",
            "A managed shortcut has an unsafe or invalid destination path.",
            Some(relative_path.to_string()),
        ));
    }
    let resolved = root.join(relative);
    if !resolved.starts_with(root) {
        return Err(AppError::new(
            "invalid_shortcut_metadata",
            "A managed shortcut path is outside the Start menu.",
            Some(relative_path.to_string()),
        ));
    }
    Ok(resolved)
}

pub fn start_menu_programs_dir() -> Result<PathBuf, AppError> {
    let app_data = std::env::var_os("APPDATA").ok_or_else(|| {
        AppError::new(
            "start_menu_unavailable",
            "The current user's Start menu folder could not be determined.",
            None,
        )
    })?;
    Ok(PathBuf::from(app_data)
        .join("Microsoft")
        .join("Windows")
        .join("Start Menu")
        .join("Programs"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_shortcut_record() -> ShortcutRecord {
        ShortcutRecord {
            id: "99314857-fcfa-4f1a-8fb2-088f24e95a3f".to_string(),
            name: "Add-ons (Work)".to_string(),
            website_name: "Add-ons".to_string(),
            target_url: "https://addons.mozilla.org/".to_string(),
            installation_id: "firefox".to_string(),
            profile_id: "default".to_string(),
            profile_path: "C:\\Firefox Profiles\\Work".to_string(),
            profile_name: "Work".to_string(),
            container_id: Some("firefox-container-7".to_string()),
            container_name: Some("Work".to_string()),
            container_user_context_id: Some(7),
            taskbar_tab_id: Some("99314857-fcfa-4f1a-8fb2-088f24e95a3f".to_string()),
            open_as_app: true,
            protocol_url: "https://addons.mozilla.org/".to_string(),
            shortcut_path: "Firefox Shortcuts\\Add-ons (Work).lnk".to_string(),
            icon_path: "C:\\Icons\\addons.ico".to_string(),
            icon_background_color: None,
            icon_used_fallback: false,
            icon_warning: None,
            created_at: 1,
        }
    }

    #[test]
    fn windows_arguments_are_quoted_without_shell_interpolation() {
        assert_eq!(quote_windows_argument("hello"), "\"hello\"");
        assert_eq!(
            quote_windows_argument("C:\\Firefox Profiles\\Work\\"),
            "\"C:\\Firefox Profiles\\Work\\\\\""
        );
        assert_eq!(
            quote_windows_argument("ext+container:name=Work&url=https://example.com"),
            "\"ext+container:name=Work&url=https://example.com\""
        );
        assert_eq!(
            quote_windows_argument("quote\"inside"),
            "\"quote\\\"inside\""
        );
    }

    #[test]
    fn rejects_unsafe_shortcut_names() {
        assert!(validate_shortcut_name("Teams (Work)").is_ok());
        assert!(validate_shortcut_name("CON").is_err());
        assert!(validate_shortcut_name("bad/name").is_err());
    }

    #[test]
    fn places_shortcuts_in_the_dedicated_start_menu_folder() {
        let programs_dir =
            Path::new(r"C:\Users\USER\AppData\Roaming\Microsoft\Windows\Start Menu\Programs");
        assert_eq!(
            shortcut_destination(programs_dir, "Teams (Work)"),
            programs_dir
                .join("Firefox Shortcuts")
                .join("Teams (Work).lnk")
        );
    }

    #[test]
    fn compares_managed_destinations_with_windows_path_case_rules() {
        let left = Path::new(r"C:\Users\USER\Start Menu\Firefox Shortcuts\Example.lnk");
        let right = Path::new(r"c:\users\user\start menu\firefox shortcuts\example.LNK");

        #[cfg(windows)]
        assert!(same_managed_destination(left, right));
        #[cfg(not(windows))]
        assert!(!same_managed_destination(left, right));
    }

    #[test]
    fn editing_preserves_the_shortcut_identity_and_creation_time() {
        let existing = app_shortcut_record();
        let request = CreateShortcutRequest {
            website_name: "New name".to_string(),
            target_url: "https://example.com".to_string(),
            installation_id: "firefox".to_string(),
            profile_id: "default".to_string(),
            container_id: Some("firefox-container-7".to_string()),
            open_as_app: true,
            shortcut_name: "Example".to_string(),
            icon_background_color: Some("#ffffff".to_string()),
        };
        let installation = FirefoxInstallation {
            id: "firefox".to_string(),
            name: "Firefox".to_string(),
            executable: r"C:\Program Files\Mozilla Firefox\firefox.exe".to_string(),
            version: None,
            kind: "stable".to_string(),
        };
        let container = FirefoxContainer {
            id: "firefox-container-7".to_string(),
            user_context_id: 7,
            name: "Work".to_string(),
            color: "blue".to_string(),
            icon: "briefcase".to_string(),
        };
        let profile = FirefoxProfile {
            id: "default".to_string(),
            name: "Default".to_string(),
            path: r"C:\Firefox Profiles\Default".to_string(),
            installation_id: Some("firefox".to_string()),
            is_default: true,
            containers: vec![container.clone()],
            container_state: crate::models::ResourceState::Available,
            extensions: Vec::new(),
            error: None,
        };
        let icon = IconOutcome {
            icon_path: r"C:\Icons\new.ico".to_string(),
            used_fallback: false,
            warning: None,
            preview_data_url: "data:image/png;base64,AA==".to_string(),
        };

        let updated = build_record(
            &request,
            &installation,
            &profile,
            Some(&container),
            "New name".to_string(),
            "https://example.com/".to_string(),
            "https://example.com/".to_string(),
            Path::new(r"Firefox Shortcuts\Example.lnk"),
            &icon,
            Some(&existing),
        )
        .unwrap();

        assert_eq!(updated.id, existing.id);
        assert_eq!(updated.created_at, existing.created_at);
        assert_eq!(updated.taskbar_tab_id, existing.taskbar_tab_id);
    }

    #[test]
    fn profile_launch_flags_avoid_remote_profile_fallback() {
        let arguments = build_firefox_arguments(
            "C:\\Firefox\\Profile",
            "ext+container:name=Work&url=https%3A%2F%2Fexample.com",
        );
        assert!(arguments.contains("\"--profile\""));
        assert!(arguments.contains("\"--no-remote\""));
        assert!(arguments.contains("\"--new-tab\""));
    }

    #[test]
    fn app_shortcuts_use_the_persisted_container_or_default_context() {
        let record = app_shortcut_record();
        assert_eq!(
            build_firefox_arguments_for_record(&record).unwrap(),
            r#""-taskbar-tab" "99314857-fcfa-4f1a-8fb2-088f24e95a3f" "-new-window" "https://addons.mozilla.org/" "-profile" "C:\Firefox Profiles\Work" "-container" "7""#
        );

        let mut no_container = record.clone();
        no_container.container_id = None;
        no_container.container_name = None;
        no_container.container_user_context_id = None;
        assert!(
            build_firefox_arguments_for_record(&no_container)
                .unwrap()
                .ends_with(r#""-container" "0""#)
        );

        let mut incomplete_container = record;
        incomplete_container.container_user_context_id = None;
        assert!(build_firefox_arguments_for_record(&incomplete_container).is_err());
    }

    #[test]
    fn app_shortcuts_use_the_registered_taskbar_tab_id() {
        let mut record = app_shortcut_record();
        record.taskbar_tab_id = Some("6a563568-b03c-48ee-9f4f-708c4c0cb760".to_string());

        let arguments = build_firefox_arguments_for_record(&record).unwrap();
        assert!(arguments.contains(r#""-taskbar-tab" "6a563568-b03c-48ee-9f4f-708c4c0cb760""#));
    }

    #[test]
    fn no_container_shortcuts_use_the_validated_website_url_directly() {
        assert_eq!(
            shortcut_launch_url(false, None, None, "https://example.com").unwrap(),
            "https://example.com/"
        );
        assert!(shortcut_launch_url(false, None, None, "file:///C:/secret").is_err());
        assert!(
            shortcut_launch_url(false, Some("container-id"), None, "https://example.com").is_err()
        );
        assert_eq!(
            shortcut_launch_url(
                true,
                Some("container-id"),
                Some("Work"),
                "https://example.com"
            )
            .unwrap(),
            "https://example.com/"
        );
    }
}
