use crate::errors::AppError;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::Mutex,
};
use url::Url;
use uuid::Uuid;

const REGISTRY_VERSION: u32 = 1;
const TASKBAR_TABS_DIRECTORY: &str = "taskbartabs";
const REGISTRY_FILE_NAME: &str = "taskbartabs.json";

static REGISTRY_MUTEX: Mutex<()> = Mutex::new(());

#[derive(Debug)]
pub struct TaskbarTabRegistration {
    id: String,
    profile_path: PathBuf,
    previous_contents: Option<Vec<u8>>,
    changed: bool,
}

impl TaskbarTabRegistration {
    pub fn id(&self) -> &str {
        &self.id
    }

    fn unchanged(id: String, profile_path: &Path) -> Self {
        Self {
            id,
            profile_path: profile_path.to_path_buf(),
            previous_contents: None,
            changed: false,
        }
    }

    fn changed(id: String, profile_path: &Path, previous_contents: Option<Vec<u8>>) -> Self {
        Self {
            id,
            profile_path: profile_path.to_path_buf(),
            previous_contents,
            changed: true,
        }
    }

    pub fn rollback(self) -> Result<(), AppError> {
        if !self.changed {
            return Ok(());
        }

        let _guard = REGISTRY_MUTEX.lock().map_err(|error| {
            AppError::new(
                "taskbar_tabs_lock_failed",
                "Firefox's app registration could not be restored safely.",
                Some(error.to_string()),
            )
        })?;
        let _profile_lock = acquire_profile_lock(&self.profile_path)?;
        match self.previous_contents {
            Some(contents) => write_registry_contents(&self.profile_path, &contents),
            None => match fs::remove_file(registry_path(&self.profile_path)) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
                Err(error) => Err(AppError::io(
                    "The new Firefox app registration could not be removed.",
                    error,
                )),
            },
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Registry {
    version: u32,
    taskbar_tabs: Vec<TaskbarTab>,
    #[serde(default, flatten)]
    extra: Map<String, Value>,
}

impl Default for Registry {
    fn default() -> Self {
        Self {
            version: REGISTRY_VERSION,
            taskbar_tabs: Vec::new(),
            extra: Map::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct TaskbarTab {
    id: String,
    scopes: Vec<TaskbarTabScope>,
    user_context_id: u32,
    start_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    shortcut_relative_path: Option<String>,
    #[serde(default, flatten)]
    extra: Map<String, Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct TaskbarTabScope {
    hostname: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    prefix: Option<String>,
    #[serde(default, flatten)]
    extra: Map<String, Value>,
}

pub fn register_taskbar_tab(
    profile_path: &Path,
    requested_id: &str,
    preferred_id: Option<&str>,
    name: &str,
    start_url: &str,
    user_context_id: u32,
) -> Result<TaskbarTabRegistration, AppError> {
    if !profile_path.is_dir() {
        return Err(AppError::new(
            "profile_unavailable",
            "The selected Firefox profile directory is unavailable.",
            Some(profile_path.display().to_string()),
        ));
    }

    let requested_id = normalize_id(requested_id)?;
    let preferred_id = preferred_id.map(normalize_id).transpose()?;
    let parsed_url = Url::parse(start_url).map_err(|error| {
        AppError::new(
            "invalid_taskbar_tab_url",
            "The Firefox app start URL is invalid.",
            Some(error.to_string()),
        )
    })?;
    if !matches!(parsed_url.scheme(), "http" | "https") {
        return Err(AppError::new(
            "invalid_taskbar_tab_url",
            "Firefox app windows require an HTTP or HTTPS start URL.",
            Some(start_url.to_string()),
        ));
    }
    let hostname = parsed_url.host_str().ok_or_else(|| {
        AppError::new(
            "invalid_taskbar_tab_url",
            "The Firefox app start URL must include a hostname.",
            Some(start_url.to_string()),
        )
    })?;
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::new(
            "invalid_taskbar_tab_name",
            "The Firefox app name cannot be empty.",
            None,
        ));
    }

    let _guard = REGISTRY_MUTEX.lock().map_err(|error| {
        AppError::new(
            "taskbar_tabs_lock_failed",
            "Firefox app registrations could not be updated safely.",
            Some(error.to_string()),
        )
    })?;
    let mut registry = read_registry(profile_path)?;

    if let Some(preferred_id) = preferred_id.as_deref() {
        if let Some(taskbar_tab) = registry
            .taskbar_tabs
            .iter()
            .find(|taskbar_tab| same_id(&taskbar_tab.id, preferred_id))
        {
            if taskbar_tab_matches(taskbar_tab, &parsed_url, hostname, user_context_id)
                && taskbar_tab.name.as_deref() == Some(name)
            {
                return Ok(TaskbarTabRegistration::unchanged(
                    taskbar_tab.id.clone(),
                    profile_path,
                ));
            }

            let profile_lock = acquire_profile_lock(profile_path)?;
            let (mut locked_registry, previous_contents) =
                read_registry_with_contents(profile_path)?;
            if let Some(index) = locked_registry
                .taskbar_tabs
                .iter()
                .position(|taskbar_tab| same_id(&taskbar_tab.id, preferred_id))
            {
                let taskbar_tab = &mut locked_registry.taskbar_tabs[index];
                update_taskbar_tab(taskbar_tab, &parsed_url, hostname, name, user_context_id);
                let id = taskbar_tab.id.clone();
                write_registry(profile_path, &locked_registry)?;
                drop(profile_lock);
                return Ok(TaskbarTabRegistration::changed(
                    id,
                    profile_path,
                    previous_contents,
                ));
            }
            drop(profile_lock);
            registry = locked_registry;
        }
    }

    let matching_id = matching_taskbar_tab(&registry, &parsed_url, hostname, user_context_id)
        .map(|taskbar_tab| taskbar_tab.id.clone());
    if let Some(matching_id) = matching_id {
        return Ok(TaskbarTabRegistration::unchanged(matching_id, profile_path));
    }

    let _profile_lock = acquire_profile_lock(profile_path)?;
    let (mut registry, previous_contents) = read_registry_with_contents(profile_path)?;

    if let Some(taskbar_tab) =
        matching_taskbar_tab(&registry, &parsed_url, hostname, user_context_id)
    {
        return Ok(TaskbarTabRegistration::unchanged(
            taskbar_tab.id.clone(),
            profile_path,
        ));
    }

    let existing_index = preferred_id.as_deref().and_then(|preferred_id| {
        registry
            .taskbar_tabs
            .iter()
            .position(|taskbar_tab| same_id(&taskbar_tab.id, preferred_id))
    });
    if let Some(index) = existing_index {
        let taskbar_tab = &mut registry.taskbar_tabs[index];
        update_taskbar_tab(taskbar_tab, &parsed_url, hostname, name, user_context_id);
        let id = taskbar_tab.id.clone();
        write_registry(profile_path, &registry)?;
        return Ok(TaskbarTabRegistration::changed(
            id,
            profile_path,
            previous_contents,
        ));
    }

    let candidate_id = preferred_id.as_deref().unwrap_or(&requested_id);
    let id = if registry
        .taskbar_tabs
        .iter()
        .any(|taskbar_tab| same_id(&taskbar_tab.id, candidate_id))
    {
        Uuid::new_v4().to_string()
    } else {
        candidate_id.to_string()
    };
    registry.taskbar_tabs.push(TaskbarTab {
        id: id.clone(),
        scopes: vec![TaskbarTabScope {
            hostname: hostname.to_string(),
            prefix: None,
            extra: Map::new(),
        }],
        user_context_id,
        start_url: parsed_url.to_string(),
        name: Some(name.to_string()),
        shortcut_relative_path: None,
        extra: Map::new(),
    });
    write_registry(profile_path, &registry)?;

    Ok(TaskbarTabRegistration::changed(
        id,
        profile_path,
        previous_contents,
    ))
}

fn update_taskbar_tab(
    taskbar_tab: &mut TaskbarTab,
    start_url: &Url,
    hostname: &str,
    name: &str,
    user_context_id: u32,
) {
    let mut scope_extra = Map::new();
    for scope in &taskbar_tab.scopes {
        for (key, value) in &scope.extra {
            scope_extra
                .entry(key.clone())
                .or_insert_with(|| value.clone());
        }
    }
    taskbar_tab.scopes = vec![TaskbarTabScope {
        hostname: hostname.to_string(),
        prefix: None,
        extra: scope_extra,
    }];
    taskbar_tab.user_context_id = user_context_id;
    taskbar_tab.start_url = start_url.to_string();
    taskbar_tab.name = Some(name.to_string());
}

pub fn matches_taskbar_tab(
    profile_path: &Path,
    id: &str,
    start_url: &str,
    user_context_id: u32,
) -> Result<bool, AppError> {
    let id = normalize_id(id)?;
    let start_url = Url::parse(start_url).map_err(|error| {
        AppError::new(
            "invalid_taskbar_tab_url",
            "The Firefox app start URL is invalid.",
            Some(error.to_string()),
        )
    })?;
    let Some(hostname) = start_url.host_str() else {
        return Ok(false);
    };
    let registry = read_registry(profile_path)?;
    Ok(registry
        .taskbar_tabs
        .iter()
        .find(|taskbar_tab| same_id(&taskbar_tab.id, &id))
        .is_some_and(|taskbar_tab| {
            taskbar_tab_matches(taskbar_tab, &start_url, hostname, user_context_id)
        }))
}

fn same_id(left: &str, right: &str) -> bool {
    match (Uuid::parse_str(left), Uuid::parse_str(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

fn normalize_id(id: &str) -> Result<String, AppError> {
    Uuid::parse_str(id)
        .map(|uuid| uuid.hyphenated().to_string())
        .map_err(|error| {
            AppError::new(
                "invalid_taskbar_tab_id",
                "A Firefox app shortcut must use a valid Taskbar Tab UUID.",
                Some(error.to_string()),
            )
        })
}

fn matching_taskbar_tab<'a>(
    registry: &'a Registry,
    start_url: &Url,
    hostname: &str,
    user_context_id: u32,
) -> Option<&'a TaskbarTab> {
    registry
        .taskbar_tabs
        .iter()
        .find(|taskbar_tab| taskbar_tab_matches(taskbar_tab, start_url, hostname, user_context_id))
}

fn taskbar_tab_matches(
    taskbar_tab: &TaskbarTab,
    start_url: &Url,
    hostname: &str,
    user_context_id: u32,
) -> bool {
    taskbar_tab.user_context_id == user_context_id
        && Url::parse(&taskbar_tab.start_url)
            .is_ok_and(|existing_url| existing_url.as_str() == start_url.as_str())
        && taskbar_tab.scopes.iter().any(|scope| {
            scope.hostname == hostname
                && scope
                    .prefix
                    .as_deref()
                    .is_none_or(|prefix| start_url.path().starts_with(prefix))
        })
}

fn read_registry(profile_path: &Path) -> Result<Registry, AppError> {
    read_registry_with_contents(profile_path).map(|(registry, _)| registry)
}

fn read_registry_with_contents(
    profile_path: &Path,
) -> Result<(Registry, Option<Vec<u8>>), AppError> {
    let path = registry_path(profile_path);
    let contents = match fs::read(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Ok((Registry::default(), None));
        }
        Err(error) => {
            return Err(AppError::io(
                "Firefox's Taskbar Tabs registry could not be read.",
                error,
            ));
        }
    };

    let registry: Registry = serde_json::from_slice(&contents).map_err(|error| {
        AppError::new(
            "invalid_taskbar_tabs_registry",
            "Firefox's Taskbar Tabs registry is invalid and was left unchanged.",
            Some(error.to_string()),
        )
    })?;
    validate_registry(&registry)?;
    Ok((registry, Some(contents)))
}

fn validate_registry(registry: &Registry) -> Result<(), AppError> {
    if registry.version != REGISTRY_VERSION {
        return Err(AppError::new(
            "unsupported_taskbar_tabs_registry",
            "This Firefox Taskbar Tabs registry version is not supported and was left unchanged.",
            Some(format!(
                "Supported version: {REGISTRY_VERSION}; registry version: {}",
                registry.version
            )),
        ));
    }

    for taskbar_tab in &registry.taskbar_tabs {
        if Uuid::parse_str(&taskbar_tab.id).is_err()
            || taskbar_tab.scopes.is_empty()
            || taskbar_tab
                .scopes
                .iter()
                .any(|scope| scope.hostname.trim().is_empty())
            || Url::parse(&taskbar_tab.start_url).is_err()
        {
            return Err(AppError::new(
                "invalid_taskbar_tabs_registry",
                "Firefox's Taskbar Tabs registry contains an invalid app entry and was left unchanged.",
                Some(taskbar_tab.id.clone()),
            ));
        }
    }
    Ok(())
}

fn write_registry(profile_path: &Path, registry: &Registry) -> Result<(), AppError> {
    let contents = serde_json::to_vec_pretty(registry).map_err(|error| {
        AppError::new(
            "taskbar_tabs_registry_save_failed",
            "Firefox's Taskbar Tabs registry could not be encoded.",
            Some(error.to_string()),
        )
    })?;
    write_registry_contents(profile_path, &contents)
}

fn write_registry_contents(profile_path: &Path, contents: &[u8]) -> Result<(), AppError> {
    let path = registry_path(profile_path);
    let parent = path.parent().ok_or_else(|| {
        AppError::new(
            "taskbar_tabs_registry_path_invalid",
            "Firefox's Taskbar Tabs registry path is invalid.",
            Some(path.display().to_string()),
        )
    })?;
    fs::create_dir_all(parent).map_err(|error| {
        AppError::io(
            "Firefox's Taskbar Tabs registry directory could not be created.",
            error,
        )
    })?;

    let temp_path = parent.join(format!(".taskbartabs.{}.tmp", Uuid::new_v4()));
    if let Err(error) = fs::write(&temp_path, contents) {
        let _ = fs::remove_file(&temp_path);
        return Err(AppError::io(
            "Firefox's Taskbar Tabs registry could not be written.",
            error,
        ));
    }
    if let Err(error) = fs::rename(&temp_path, &path) {
        let _ = fs::remove_file(&temp_path);
        return Err(AppError::io(
            "Firefox's Taskbar Tabs registry could not be saved atomically.",
            error,
        ));
    }
    Ok(())
}

fn registry_path(profile_path: &Path) -> PathBuf {
    profile_path
        .join(TASKBAR_TABS_DIRECTORY)
        .join(REGISTRY_FILE_NAME)
}

#[cfg(windows)]
pub(crate) fn acquire_profile_lock(profile_path: &Path) -> Result<fs::File, AppError> {
    use std::os::windows::fs::OpenOptionsExt;

    let lock_path = profile_path.join("parent.lock");
    // Hold Firefox's own exclusive lock through the registry replacement.
    match fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .share_mode(0)
        .open(&lock_path)
    {
        Ok(file) => Ok(file),
        Err(error) => Err(AppError::new(
            "firefox_profile_in_use",
            "Close Firefox for the selected profile before changing its app-window settings.",
            Some(format!("{}: {error}", lock_path.display())),
        )),
    }
}

#[cfg(not(windows))]
pub(crate) fn acquire_profile_lock(_profile_path: &Path) -> Result<(), AppError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const REQUESTED_ID: &str = "99314857-fcfa-4f1a-8fb2-088f24e95a3f";

    fn temporary_profile() -> PathBuf {
        let profile = std::env::temp_dir().join(format!("fsm-taskbar-tabs-{}", Uuid::new_v4()));
        fs::create_dir_all(&profile).unwrap();
        profile
    }

    #[test]
    fn creates_a_registry_entry_for_the_requested_container() {
        let profile = temporary_profile();
        let registration = register_taskbar_tab(
            &profile,
            REQUESTED_ID,
            None,
            "Add-ons",
            "https://addons.mozilla.org/",
            7,
        )
        .unwrap();

        assert_eq!(registration.id(), REQUESTED_ID);
        assert!(
            matches_taskbar_tab(&profile, REQUESTED_ID, "https://addons.mozilla.org/", 7).unwrap()
        );
        assert!(
            !matches_taskbar_tab(&profile, REQUESTED_ID, "https://addons.mozilla.org/", 0).unwrap()
        );
        let registry: Value =
            serde_json::from_slice(&fs::read(registry_path(&profile)).unwrap()).unwrap();
        let entry = &registry["taskbarTabs"][0];
        assert_eq!(entry["id"], REQUESTED_ID);
        assert_eq!(entry["scopes"][0]["hostname"], "addons.mozilla.org");
        assert_eq!(entry["userContextId"], 7);
        assert_eq!(entry["startUrl"], "https://addons.mozilla.org/");
        assert_eq!(entry["name"], "Add-ons");

        let repeated_registration = register_taskbar_tab(
            &profile,
            REQUESTED_ID,
            None,
            "Add-ons",
            "https://addons.mozilla.org/",
            7,
        )
        .unwrap();
        assert_eq!(repeated_registration.id(), registration.id());
        let registry: Value =
            serde_json::from_slice(&fs::read(registry_path(&profile)).unwrap()).unwrap();
        assert_eq!(registry["taskbarTabs"].as_array().unwrap().len(), 1);

        let _ = fs::remove_dir_all(profile);
    }

    #[test]
    fn reuses_exact_registrations_and_preserves_unknown_registry_fields() {
        let profile = temporary_profile();
        let existing_id = "6a563568-b03c-48ee-9f4f-708c4c0cb760";
        let registry = serde_json::json!({
            "version": 1,
            "futureField": "preserve",
            "taskbarTabs": [{
                "id": existing_id,
                "scopes": [{
                    "hostname": "addons.mozilla.org",
                    "futureScopeField": true
                }],
                "userContextId": 7,
                "startUrl": "https://addons.mozilla.org",
                "name": "Mozilla Add-ons",
                "futureTabField": "preserve"
            }]
        });
        fs::create_dir_all(profile.join(TASKBAR_TABS_DIRECTORY)).unwrap();
        let original_contents = serde_json::to_vec_pretty(&registry).unwrap();
        fs::write(registry_path(&profile), &original_contents).unwrap();

        let registration = register_taskbar_tab(
            &profile,
            REQUESTED_ID,
            None,
            "Add-ons",
            "https://addons.mozilla.org/",
            7,
        )
        .unwrap();

        assert_eq!(registration.id(), existing_id);
        let updated: Value =
            serde_json::from_slice(&fs::read(registry_path(&profile)).unwrap()).unwrap();
        assert_eq!(updated["futureField"], "preserve");
        assert_eq!(updated["taskbarTabs"].as_array().unwrap().len(), 1);
        assert_eq!(updated["taskbarTabs"][0]["futureTabField"], "preserve");
        assert_eq!(
            updated["taskbarTabs"][0]["scopes"][0]["futureScopeField"],
            true
        );

        let _ = fs::remove_dir_all(profile);
    }

    #[test]
    fn updates_a_preferred_registration_without_changing_its_id() {
        let profile = temporary_profile();
        let registration = register_taskbar_tab(
            &profile,
            REQUESTED_ID,
            None,
            "Add-ons",
            "https://addons.mozilla.org/",
            0,
        )
        .unwrap();
        let id = registration.id().to_string();

        let updated_registration = register_taskbar_tab(
            &profile,
            "c527c2e0-cf9d-4c09-95af-4930951b3309",
            Some(&id),
            "Developer Add-ons",
            "https://addons.mozilla.org/developers/",
            7,
        )
        .unwrap();

        assert_eq!(updated_registration.id(), id.as_str());
        let registry: Value =
            serde_json::from_slice(&fs::read(registry_path(&profile)).unwrap()).unwrap();
        assert_eq!(registry["taskbarTabs"].as_array().unwrap().len(), 1);
        assert_eq!(
            registry["taskbarTabs"][0]["startUrl"],
            "https://addons.mozilla.org/developers/"
        );
        assert_eq!(registry["taskbarTabs"][0]["userContextId"], 7);
        assert_eq!(registry["taskbarTabs"][0]["name"], "Developer Add-ons");

        register_taskbar_tab(
            &profile,
            "c527c2e0-cf9d-4c09-95af-4930951b3309",
            Some(&id),
            "Updated name",
            "https://addons.mozilla.org/developers/",
            7,
        )
        .unwrap();
        let registry: Value =
            serde_json::from_slice(&fs::read(registry_path(&profile)).unwrap()).unwrap();
        assert_eq!(registry["taskbarTabs"][0]["name"], "Updated name");

        let _ = fs::remove_dir_all(profile);
    }

    #[test]
    fn preserves_unknown_fields_when_updating_a_preferred_registration() {
        let profile = temporary_profile();
        let existing_id = "6a563568-b03c-48ee-9f4f-708c4c0cb760";
        let registry = serde_json::json!({
            "version": 1,
            "futureField": "preserve",
            "taskbarTabs": [{
                "id": existing_id,
                "scopes": [{
                    "hostname": "addons.mozilla.org",
                    "futureScopeField": true
                }],
                "userContextId": 0,
                "startUrl": "https://addons.mozilla.org/",
                "name": "Add-ons",
                "futureTabField": "preserve"
            }]
        });
        fs::create_dir_all(profile.join(TASKBAR_TABS_DIRECTORY)).unwrap();
        let original_contents = serde_json::to_vec_pretty(&registry).unwrap();
        fs::write(registry_path(&profile), &original_contents).unwrap();

        let registration = register_taskbar_tab(
            &profile,
            REQUESTED_ID,
            Some(existing_id),
            "Developer Add-ons",
            "https://addons.mozilla.org/developers/",
            7,
        )
        .unwrap();

        assert_eq!(registration.id(), existing_id);
        let updated: Value =
            serde_json::from_slice(&fs::read(registry_path(&profile)).unwrap()).unwrap();
        assert_eq!(updated["futureField"], "preserve");
        assert_eq!(updated["taskbarTabs"][0]["futureTabField"], "preserve");
        assert_eq!(
            updated["taskbarTabs"][0]["scopes"][0]["futureScopeField"],
            true
        );
        registration.rollback().unwrap();
        assert_eq!(
            fs::read(registry_path(&profile)).unwrap(),
            original_contents
        );
        let _ = fs::remove_dir_all(profile);
    }

    #[test]
    fn refuses_unsupported_registry_versions_without_overwriting_them() {
        let profile = temporary_profile();
        fs::create_dir_all(profile.join(TASKBAR_TABS_DIRECTORY)).unwrap();
        let path = registry_path(&profile);
        let original = br#"{"version":2,"taskbarTabs":[]}"#;
        fs::write(&path, original).unwrap();

        let error = register_taskbar_tab(
            &profile,
            REQUESTED_ID,
            None,
            "Add-ons",
            "https://addons.mozilla.org/",
            0,
        )
        .unwrap_err();

        assert_eq!(error.code, "unsupported_taskbar_tabs_registry");
        assert_eq!(fs::read(path).unwrap(), original);
        let _ = fs::remove_dir_all(profile);
    }

    #[test]
    fn refuses_malformed_registry_data_without_overwriting_it() {
        let profile = temporary_profile();
        fs::create_dir_all(profile.join(TASKBAR_TABS_DIRECTORY)).unwrap();
        let path = registry_path(&profile);
        let original = b"not-json";
        fs::write(&path, original).unwrap();

        let error = register_taskbar_tab(
            &profile,
            REQUESTED_ID,
            None,
            "Add-ons",
            "https://addons.mozilla.org/",
            0,
        )
        .unwrap_err();

        assert_eq!(error.code, "invalid_taskbar_tabs_registry");
        assert_eq!(fs::read(path).unwrap(), original);
        let _ = fs::remove_dir_all(profile);
    }

    #[test]
    fn rollback_removes_a_new_registry_entry() {
        let profile = temporary_profile();
        let registration = register_taskbar_tab(
            &profile,
            REQUESTED_ID,
            None,
            "Add-ons",
            "https://addons.mozilla.org/",
            0,
        )
        .unwrap();

        registration.rollback().unwrap();

        assert!(!registry_path(&profile).exists());
        let _ = fs::remove_dir_all(profile);
    }

    #[test]
    fn rollback_restores_the_original_registry_bytes() {
        let profile = temporary_profile();
        fs::create_dir_all(profile.join(TASKBAR_TABS_DIRECTORY)).unwrap();
        let path = registry_path(&profile);
        let original = br#"{
  "version": 1,
  "taskbarTabs": []
}"#;
        fs::write(&path, original).unwrap();
        let registration = register_taskbar_tab(
            &profile,
            REQUESTED_ID,
            None,
            "Add-ons",
            "https://addons.mozilla.org/",
            0,
        )
        .unwrap();

        registration.rollback().unwrap();

        assert_eq!(fs::read(path).unwrap(), original);
        let _ = fs::remove_dir_all(profile);
    }

    #[cfg(windows)]
    #[test]
    fn does_not_write_when_firefox_holds_the_profile_lock() {
        use std::os::windows::fs::OpenOptionsExt;

        let profile = temporary_profile();
        let profile_lock = fs::OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .share_mode(0)
            .open(profile.join("parent.lock"))
            .unwrap();

        let error = register_taskbar_tab(
            &profile,
            REQUESTED_ID,
            None,
            "Add-ons",
            "https://addons.mozilla.org/",
            0,
        )
        .unwrap_err();

        assert_eq!(error.code, "firefox_profile_in_use");
        assert!(!registry_path(&profile).exists());
        drop(profile_lock);
        let _ = fs::remove_dir_all(profile);
    }

    #[cfg(windows)]
    #[test]
    fn reuses_an_existing_registration_while_firefox_holds_the_profile_lock() {
        use std::os::windows::fs::OpenOptionsExt;

        let profile = temporary_profile();
        let first = register_taskbar_tab(
            &profile,
            REQUESTED_ID,
            None,
            "Add-ons",
            "https://addons.mozilla.org/",
            7,
        )
        .unwrap();
        let existing_id = first.id().to_string();
        drop(first);

        let profile_lock = fs::OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .share_mode(0)
            .open(profile.join("parent.lock"))
            .unwrap();
        let reused = register_taskbar_tab(
            &profile,
            "c527c2e0-cf9d-4c09-95af-4930951b3309",
            None,
            "Different display name",
            "https://addons.mozilla.org/",
            7,
        )
        .unwrap();

        assert_eq!(reused.id(), existing_id);
        drop(profile_lock);
        let _ = fs::remove_dir_all(profile);
    }
}
