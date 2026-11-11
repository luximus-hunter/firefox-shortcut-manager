use crate::models::{
    AppSettings, DetectionIssue, ExtensionDetectionState, ExtensionStatus, FirefoxContainer,
    FirefoxInstallation, FirefoxProfile, ResourceState, ScanResult,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

const PROFILE_ROOT: &str = "Mozilla\\Firefox";

struct IniSection {
    name: String,
    values: BTreeMap<String, String>,
}

struct ParsedProfile {
    name: String,
    path: PathBuf,
    is_default: bool,
}

struct ExtensionDefinition {
    id: &'static str,
    name: &'static str,
    purpose: &'static str,
    required: bool,
    download_url: &'static str,
}

const KNOWN_EXTENSIONS: [ExtensionDefinition; 3] = [
    ExtensionDefinition {
        id: "{f069aec0-43c5-4bbf-b6b4-df95c4326b98}",
        name: "Open external links in a container",
        purpose: "Handles ext+container links used by shortcuts.",
        required: true,
        download_url: "https://addons.mozilla.org/en-US/firefox/addon/open-url-in-container/",
    },
    ExtensionDefinition {
        id: "{572d2b62-7dda-4e5e-b71f-e6480f4bd5cd}",
        name: "Container Windows RE",
        purpose: "Groups container tabs into dedicated Firefox windows and skips detected app windows.",
        required: true,
        download_url: "https://addons.mozilla.org/en-US/firefox/addon/container-windows-re/",
    },
    ExtensionDefinition {
        id: "persistent-containers-mv3@navchandar.github.io",
        name: "Persistent Containers",
        purpose: "Keeps ordinary new tabs in the current container.",
        required: false,
        download_url: "https://addons.mozilla.org/en-US/firefox/addon/persitent-containers/",
    },
];

pub fn scan_firefox(settings: &AppSettings) -> ScanResult {
    let mut diagnostics = Vec::new();
    let installations = discover_installations(&settings.custom_firefox_paths, &mut diagnostics);
    let profiles = discover_profiles(&mut diagnostics);

    if installations.is_empty() {
        diagnostics.push(DetectionIssue {
            source: "Firefox installations".to_string(),
            message: "No Firefox executable was found. Add a custom executable path in Settings."
                .to_string(),
        });
    }

    ScanResult {
        installations,
        profiles,
        diagnostics,
    }
}

fn discover_installations(
    custom_paths: &[String],
    diagnostics: &mut Vec<DetectionIssue>,
) -> Vec<FirefoxInstallation> {
    let mut candidates = Vec::<(PathBuf, bool)>::new();
    for path in custom_paths {
        candidates.push((PathBuf::from(path), true));
    }

    #[cfg(windows)]
    candidates.extend(registry_installation_paths());

    candidates.extend(known_installation_paths());

    let mut installations = Vec::new();
    let mut seen = HashSet::new();

    for (candidate, is_custom) in candidates {
        let executable = if candidate.is_dir() {
            candidate.join("firefox.exe")
        } else {
            candidate
        };

        if !executable.is_file() {
            if is_custom {
                diagnostics.push(DetectionIssue {
                    source: "Custom Firefox installation".to_string(),
                    message: format!(
                        "The configured Firefox executable was not found: {}",
                        executable.display()
                    ),
                });
            }
            continue;
        }

        let key = normalized_path(&executable);
        if !seen.insert(key) {
            continue;
        }

        let kind = installation_kind(&executable, is_custom);
        installations.push(FirefoxInstallation {
            id: stable_id("installation", &executable),
            name: installation_name(&kind, &executable),
            executable: executable.to_string_lossy().into_owned(),
            version: read_firefox_version(&executable),
            kind,
        });
    }

    installations.sort_by(|left, right| left.name.cmp(&right.name));
    installations
}

fn known_installation_paths() -> Vec<(PathBuf, bool)> {
    let mut candidates = Vec::new();
    let mut roots = Vec::new();

    for variable in ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"] {
        if let Some(root) = std::env::var_os(variable) {
            roots.push(PathBuf::from(root));
        }
    }

    for root in roots {
        let program_dirs = [
            "Mozilla Firefox",
            "Mozilla Firefox ESR",
            "Firefox Developer Edition",
            "Firefox",
        ];
        for directory in program_dirs {
            candidates.push((root.join(directory).join("firefox.exe"), false));
        }

        candidates.push((
            root.join("Programs")
                .join("Mozilla Firefox")
                .join("firefox.exe"),
            false,
        ));
        candidates.push((
            root.join("Programs")
                .join("Firefox Developer Edition")
                .join("firefox.exe"),
            false,
        ));
    }

    candidates
}

#[cfg(windows)]
fn registry_installation_paths() -> Vec<(PathBuf, bool)> {
    use winreg::{
        RegKey,
        enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE},
    };

    let mut candidates = Vec::new();
    let registry_roots = [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE];
    let registry_paths = [
        "SOFTWARE\\Mozilla\\Mozilla Firefox",
        "SOFTWARE\\Mozilla\\Mozilla Firefox ESR",
        "SOFTWARE\\Mozilla\\Firefox",
        "SOFTWARE\\WOW6432Node\\Mozilla\\Mozilla Firefox",
        "SOFTWARE\\WOW6432Node\\Mozilla\\Mozilla Firefox ESR",
        "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\App Paths\\firefox.exe",
        "SOFTWARE\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\App Paths\\firefox.exe",
    ];

    for registry_root in registry_roots {
        let root = RegKey::predef(registry_root);
        for key_path in registry_paths {
            let Ok(key) = root.open_subkey(key_path) else {
                continue;
            };

            for value_name in ["PathToExe", ""] {
                if let Ok(value) = key.get_value::<String, _>(value_name) {
                    candidates.push((executable_from_registry_command(&value), false));
                }
            }

            for value_name in ["Install Directory", "InstallDir", "Path"] {
                if let Ok(value) = key.get_value::<String, _>(value_name) {
                    candidates.push((PathBuf::from(value).join("firefox.exe"), false));
                }
            }

            if let Ok(current_version) = key.get_value::<String, _>("CurrentVersion") {
                if let Ok(version_key) = key.open_subkey(current_version) {
                    if let Ok(main_key) = version_key.open_subkey("Main") {
                        if let Ok(directory) = main_key.get_value::<String, _>("Install Directory")
                        {
                            candidates.push((PathBuf::from(directory).join("firefox.exe"), false));
                        }
                        if let Ok(executable) = main_key.get_value::<String, _>("PathToExe") {
                            candidates.push((PathBuf::from(executable), false));
                        }
                    }
                }
            }

            if let Ok(subkeys) = key.enum_keys().collect::<Result<Vec<_>, _>>() {
                for subkey in subkeys {
                    if let Ok(version_key) = key.open_subkey(subkey) {
                        if let Ok(main_key) = version_key.open_subkey("Main") {
                            if let Ok(directory) =
                                main_key.get_value::<String, _>("Install Directory")
                            {
                                candidates
                                    .push((PathBuf::from(directory).join("firefox.exe"), false));
                            }
                            if let Ok(executable) = main_key.get_value::<String, _>("PathToExe") {
                                candidates.push((PathBuf::from(executable), false));
                            }
                        }
                    }
                }
            }
        }
    }

    candidates
}

#[cfg(windows)]
fn executable_from_registry_command(value: &str) -> PathBuf {
    let value = value.trim();
    if let Some(quoted) = value.strip_prefix('"') {
        if let Some((executable, _)) = quoted.split_once('"') {
            return PathBuf::from(executable);
        }
    }
    PathBuf::from(value.split_whitespace().next().unwrap_or(value))
}

fn installation_kind(executable: &Path, is_custom: bool) -> String {
    let path = executable.to_string_lossy().to_ascii_lowercase();
    if path.contains("developer edition") || path.contains("firefox developeredition") {
        "developer".to_string()
    } else if path.contains("esr") {
        "esr".to_string()
    } else if is_custom {
        if path.contains("portable") {
            "portable".to_string()
        } else {
            "custom".to_string()
        }
    } else {
        "standard".to_string()
    }
}

fn installation_name(kind: &str, executable: &Path) -> String {
    match kind {
        "developer" => "Firefox Developer Edition".to_string(),
        "esr" => "Firefox ESR".to_string(),
        "portable" => "Portable Firefox".to_string(),
        "custom" => executable
            .parent()
            .and_then(Path::file_name)
            .map(|name| format!("Firefox ({})", name.to_string_lossy()))
            .unwrap_or_else(|| "Custom Firefox".to_string()),
        _ => "Firefox".to_string(),
    }
}

fn read_firefox_version(executable: &Path) -> Option<String> {
    let application_ini = executable.parent()?.join("application.ini");
    let contents = fs::read_to_string(application_ini).ok()?;
    let sections = parse_ini(&contents).ok()?;
    sections
        .iter()
        .find(|section| section.name.eq_ignore_ascii_case("App"))?
        .values
        .get("Version")
        .cloned()
}

fn discover_profiles(diagnostics: &mut Vec<DetectionIssue>) -> Vec<FirefoxProfile> {
    let Some(app_data) = std::env::var_os("APPDATA") else {
        diagnostics.push(DetectionIssue {
            source: "Firefox profiles".to_string(),
            message: "The Windows APPDATA directory could not be determined.".to_string(),
        });
        return Vec::new();
    };

    let root = PathBuf::from(app_data).join(PROFILE_ROOT);
    let profiles_ini = root.join("profiles.ini");
    let contents = match fs::read_to_string(&profiles_ini) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            diagnostics.push(DetectionIssue {
                source: "Firefox profiles".to_string(),
                message: format!(
                    "Firefox profile configuration was not found at {}.",
                    profiles_ini.display()
                ),
            });
            return Vec::new();
        }
        Err(error) => {
            diagnostics.push(DetectionIssue {
                source: "Firefox profiles".to_string(),
                message: format!("Firefox profile configuration could not be read: {}", error),
            });
            return Vec::new();
        }
    };

    let installs_ini = root.join("installs.ini");
    let installs = match fs::read_to_string(&installs_ini) {
        Ok(contents) => Some(contents),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            diagnostics.push(DetectionIssue {
                source: "Firefox profiles".to_string(),
                message: format!("Firefox installation profile data could not be read: {error}"),
            });
            None
        }
    };

    let (parsed_profiles, parse_errors) =
        match parse_profile_ini(&contents, &root, installs.as_deref()) {
            Ok(result) => result,
            Err(error) => {
                diagnostics.push(DetectionIssue {
                    source: "Firefox profiles".to_string(),
                    message: format!("Firefox profile configuration is malformed: {error}"),
                });
                return Vec::new();
            }
        };

    diagnostics.extend(parse_errors.into_iter().map(|message| DetectionIssue {
        source: "Firefox profiles".to_string(),
        message,
    }));

    parsed_profiles
        .into_iter()
        .map(|profile| scan_profile(profile, diagnostics))
        .collect()
}

fn scan_profile(profile: ParsedProfile, diagnostics: &mut Vec<DetectionIssue>) -> FirefoxProfile {
    let path = profile.path;
    let id = stable_id("profile", &path);
    let mut containers = Vec::new();
    let mut container_state = ResourceState::NotAvailable;
    let mut extensions = detection_failed_extensions();
    let mut error = None;

    if path.is_dir() {
        let containers_path = path.join("containers.json");
        match fs::read_to_string(&containers_path) {
            Ok(contents) => match parse_containers(&contents) {
                Ok(found) => {
                    container_state = if found.is_empty() {
                        ResourceState::Empty
                    } else {
                        ResourceState::Available
                    };
                    containers = found;
                }
                Err(parse_error) => {
                    container_state = ResourceState::Failed;
                    error = Some(format!("Container data could not be parsed: {parse_error}"));
                }
            },
            Err(read_error) if read_error.kind() == std::io::ErrorKind::NotFound => {
                container_state = ResourceState::Empty;
            }
            Err(read_error) => {
                container_state = ResourceState::Failed;
                error = Some(format!("Container data could not be read: {read_error}"));
            }
        }

        extensions = match fs::read_to_string(path.join("extensions.json")) {
            Ok(contents) => match parse_extensions(&contents) {
                Ok(found) => found,
                Err(parse_error) => {
                    diagnostics.push(DetectionIssue {
                        source: profile.name.clone(),
                        message: format!("Extension metadata could not be parsed: {parse_error}"),
                    });
                    detection_failed_extensions()
                }
            },
            Err(read_error) if read_error.kind() == std::io::ErrorKind::NotFound => {
                detection_failed_extensions()
            }
            Err(read_error) => {
                diagnostics.push(DetectionIssue {
                    source: profile.name.clone(),
                    message: format!("Extension metadata could not be read: {read_error}"),
                });
                detection_failed_extensions()
            }
        };
    } else {
        error = Some("The Firefox profile directory no longer exists.".to_string());
        diagnostics.push(DetectionIssue {
            source: profile.name.clone(),
            message: error.clone().unwrap(),
        });
    }

    if let Some(message) = &error {
        diagnostics.push(DetectionIssue {
            source: profile.name.clone(),
            message: message.clone(),
        });
    }

    FirefoxProfile {
        id,
        name: profile.name,
        path: path.to_string_lossy().into_owned(),
        installation_id: None,
        is_default: profile.is_default,
        containers,
        container_state,
        extensions,
        error,
    }
}

fn parse_ini(contents: &str) -> Result<Vec<IniSection>, String> {
    let mut sections = Vec::new();
    let mut current: Option<IniSection> = None;

    for (line_number, raw_line) in contents.lines().enumerate() {
        let line = raw_line.trim().trim_start_matches('\u{feff}').trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') && line.len() > 2 {
            if let Some(section) = current.take() {
                sections.push(section);
            }
            current = Some(IniSection {
                name: line[1..line.len() - 1].trim().to_string(),
                values: BTreeMap::new(),
            });
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            return Err(format!("Invalid entry on line {}.", line_number + 1));
        };
        let Some(section) = current.as_mut() else {
            return Err(format!(
                "Entry before the first section on line {}.",
                line_number + 1
            ));
        };
        section
            .values
            .insert(key.trim().to_string(), value.trim().to_string());
    }

    if let Some(section) = current {
        sections.push(section);
    }
    Ok(sections)
}

fn parse_profile_ini(
    contents: &str,
    root: &Path,
    installs_ini: Option<&str>,
) -> Result<(Vec<ParsedProfile>, Vec<String>), String> {
    let sections = parse_ini(contents)?;
    let mut errors = Vec::new();
    let mut default_paths = HashSet::new();

    for section in sections
        .iter()
        .filter(|section| section.name.starts_with("Install"))
    {
        if let Some(default_path) = section.values.get("Default") {
            default_paths.insert(normalized_path(&resolve_profile_path(
                root,
                default_path,
                true,
            )));
        }
    }

    if let Some(installs_ini) = installs_ini {
        match parse_ini(installs_ini) {
            Ok(install_sections) => {
                for section in install_sections {
                    if let Some(default_path) = section.values.get("Default") {
                        default_paths.insert(normalized_path(&resolve_profile_path(
                            root,
                            default_path,
                            true,
                        )));
                    }
                }
            }
            Err(error) => errors.push(format!("installs.ini could not be parsed: {error}")),
        }
    }

    let mut profiles = Vec::new();
    for section in sections
        .iter()
        .filter(|section| section.name.starts_with("Profile"))
    {
        let Some(profile_path) = section.values.get("Path") else {
            errors.push(format!(
                "Section [{}] is missing its profile path.",
                section.name
            ));
            continue;
        };

        let is_relative = section
            .values
            .get("IsRelative")
            .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
            .unwrap_or(true);
        let path = resolve_profile_path(root, profile_path, is_relative);
        let name = section
            .values
            .get("Name")
            .cloned()
            .unwrap_or_else(|| section.name.clone());
        let is_default = section
            .values
            .get("Default")
            .is_some_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
            || default_paths.contains(&normalized_path(&path));

        profiles.push(ParsedProfile {
            name,
            path,
            is_default,
        });
    }

    Ok((profiles, errors))
}

fn resolve_profile_path(root: &Path, value: &str, is_relative: bool) -> PathBuf {
    let path = PathBuf::from(value);
    if is_relative && path.is_relative() {
        root.join(path)
    } else {
        path
    }
}

fn parse_containers(contents: &str) -> Result<Vec<FirefoxContainer>, String> {
    let value: Value = serde_json::from_str(contents).map_err(|error| error.to_string())?;
    let identities = value
        .get("identities")
        .and_then(Value::as_array)
        .ok_or_else(|| "The identities list is missing.".to_string())?;
    let mut containers = Vec::new();

    for identity in identities {
        // Firefox stores private implementation contexts alongside user containers.
        if identity.get("public").and_then(Value::as_bool) == Some(false)
            || ["name", "l10nID"].iter().any(|key| {
                identity.get(*key).and_then(Value::as_str)
                    .is_some_and(|value| value.starts_with("userContextIdInternal"))
            })
        {
            continue;
        }
        let Some(user_context_id) = identity
            .get("userContextId")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
        else {
            continue;
        };
        let Some(name) = identity.get("name").and_then(Value::as_str) else {
            continue;
        };
        if name.trim().is_empty() {
            continue;
        }

        containers.push(FirefoxContainer {
            id: format!("firefox-container-{user_context_id}"),
            user_context_id,
            name: name.to_string(),
            color: identity
                .get("color")
                .and_then(Value::as_str)
                .unwrap_or("blue")
                .to_string(),
            icon: identity
                .get("icon")
                .and_then(Value::as_str)
                .unwrap_or("fingerprint")
                .to_string(),
        });
    }

    containers.sort_by_key(|container| container.user_context_id);
    Ok(containers)
}

fn parse_extensions(contents: &str) -> Result<Vec<ExtensionStatus>, String> {
    let value: Value = serde_json::from_str(contents).map_err(|error| error.to_string())?;
    let addons = value
        .get("addons")
        .and_then(Value::as_array)
        .ok_or_else(|| "The add-ons list is missing.".to_string())?;

    Ok(KNOWN_EXTENSIONS
        .iter()
        .map(|definition| {
            let metadata = addons.iter().find(|addon| {
                addon.get("id").and_then(Value::as_str).is_some_and(|id| {
                    normalize_extension_id(id) == normalize_extension_id(definition.id)
                })
            });

            let (status, version) = match metadata {
                None => (ExtensionDetectionState::NotInstalled, None),
                Some(addon) => {
                    let user_disabled = addon
                        .get("userDisabled")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    let app_disabled = addon
                        .get("appDisabled")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    let active = addon.get("active").and_then(Value::as_bool);
                    let status = if user_disabled || app_disabled {
                        ExtensionDetectionState::InstalledDisabled
                    } else if active == Some(true) {
                        ExtensionDetectionState::InstalledEnabled
                    } else {
                        ExtensionDetectionState::InstalledUnknown
                    };
                    let version = addon
                        .get("version")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                    (status, version)
                }
            };

            ExtensionStatus {
                id: definition.id.to_string(),
                name: definition.name.to_string(),
                purpose: definition.purpose.to_string(),
                status,
                version,
                required: definition.required,
                download_url: definition.download_url.to_string(),
            }
        })
        .collect())
}

fn detection_failed_extensions() -> Vec<ExtensionStatus> {
    KNOWN_EXTENSIONS
        .iter()
        .map(|definition| ExtensionStatus {
            id: definition.id.to_string(),
            name: definition.name.to_string(),
            purpose: definition.purpose.to_string(),
            status: ExtensionDetectionState::DetectionFailed,
            version: None,
            required: definition.required,
            download_url: definition.download_url.to_string(),
        })
        .collect()
}

fn normalize_extension_id(id: &str) -> String {
    id.trim()
        .trim_start_matches('{')
        .trim_end_matches('}')
        .to_ascii_lowercase()
}

fn stable_id(kind: &str, path: &Path) -> String {
    let normalized = normalized_path(path);
    let digest = Sha256::digest(normalized.as_bytes());
    format!("{kind}-{}", hex_prefix(&digest, 12))
}

fn hex_prefix(bytes: &[u8], length: usize) -> String {
    bytes
        .iter()
        .take(length)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn normalized_path(path: &Path) -> String {
    let path = path.to_string_lossy().replace('/', "\\");
    if cfg!(windows) {
        path.to_ascii_lowercase()
    } else {
        path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_relative_and_absolute_profile_paths_and_default_install() {
        let root = PathBuf::from("C:\\Users\\A\\AppData\\Roaming\\Mozilla\\Firefox");
        let profiles = r#"
[Profile0]
Name=Work
IsRelative=1
Path=Profiles/work.default

[Profile1]
Name=School
IsRelative=0
Path=D:\Firefox Profiles\school

[InstallABC]
Default=Profiles/work.default
"#;
        let (parsed, errors) = parse_profile_ini(profiles, &root, None).unwrap();

        assert!(errors.is_empty());
        assert_eq!(parsed[0].path, root.join("Profiles").join("work.default"));
        assert!(parsed[0].is_default);
        assert_eq!(
            parsed[1].path,
            PathBuf::from("D:\\Firefox Profiles\\school")
        );
        assert!(!parsed[1].is_default);
    }

    #[test]
    fn reports_profiles_missing_a_path_without_dropping_valid_profiles() {
        let root = PathBuf::from("C:\\Firefox");
        let profiles = r#"
[Profile0]
Name=Broken

[Profile1]
Name=Valid
Path=Profiles\valid
IsRelative=1
"#;

        let (parsed, errors) = parse_profile_ini(profiles, &root, None).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].name, "Valid");
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("Profile0"));
    }

    #[test]
    fn reads_existing_containers_without_writing_profile_data() {
        let containers = r#"{
          "identities": [
            {"userContextId": 2, "name": "Work & Équipe", "color": "blue", "icon": "briefcase"}
          ]
        }"#;

        let parsed = parse_containers(containers).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].id, "firefox-container-2");
        assert_eq!(parsed[0].name, "Work & Équipe");
        assert_eq!(parsed[0].color, "blue");
    }

    #[test]
    fn excludes_internal_contexts_from_user_containers() {
        let parsed = parse_containers(r#"{"identities":[
            {"userContextId":1,"name":"Personal","public":true},
            {"userContextId":2,"name":"Work"},
            {"userContextId":3,"name":"Private implementation","public":false},
            {"userContextId":4,"name":"userContextIdInternal.thumbnail"},
            {"userContextId":5,"name":"Internal","l10nID":"userContextIdInternal.webextStorageLocal"}
        ]}"#).unwrap();
        assert_eq!(parsed.iter().map(|container| container.user_context_id).collect::<Vec<_>>(), vec![1, 2]);
    }

    #[test]
    fn distinguishes_installed_disabled_and_missing_extensions() {
        let metadata = format!(
            r#"{{"addons":[
              {{"id":"{}","version":"1.0","active":true,"userDisabled":false}},
              {{"id":"{}","version":"2.0","active":false,"userDisabled":true}}
            ]}}"#,
            KNOWN_EXTENSIONS[0].id, KNOWN_EXTENSIONS[1].id
        );

        let parsed = parse_extensions(&metadata).unwrap();
        assert!(matches!(
            parsed[0].status,
            ExtensionDetectionState::InstalledEnabled
        ));
        assert!(matches!(
            parsed[1].status,
            ExtensionDetectionState::InstalledDisabled
        ));
        assert!(matches!(
            parsed[2].status,
            ExtensionDetectionState::NotInstalled
        ));
    }
}
