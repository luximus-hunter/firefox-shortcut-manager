use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub schema_version: u32,
    pub theme: ThemePreference,
    pub default_installation_id: Option<String>,
    pub default_profile_id: Option<String>,
    pub auto_scan: bool,
    pub custom_firefox_paths: Vec<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            schema_version: 1,
            theme: ThemePreference::System,
            default_installation_id: None,
            default_profile_id: None,
            auto_scan: true,
            custom_firefox_paths: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemePreference {
    System,
    Light,
    Dark,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub installations: Vec<FirefoxInstallation>,
    pub profiles: Vec<FirefoxProfile>,
    pub diagnostics: Vec<DetectionIssue>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FirefoxInstallation {
    pub id: String,
    pub name: String,
    pub executable: String,
    pub version: Option<String>,
    pub kind: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FirefoxProfile {
    pub id: String,
    pub name: String,
    pub path: String,
    pub installation_id: Option<String>,
    pub is_default: bool,
    pub containers: Vec<FirefoxContainer>,
    pub container_state: ResourceState,
    pub extensions: Vec<ExtensionStatus>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FirefoxContainer {
    pub id: String,
    pub user_context_id: u32,
    pub name: String,
    pub color: String,
    pub icon: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionStatus {
    pub id: String,
    pub name: String,
    pub purpose: String,
    pub status: ExtensionDetectionState,
    pub version: Option<String>,
    pub required: bool,
    pub download_url: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ExtensionDetectionState {
    InstalledEnabled,
    InstalledDisabled,
    InstalledUnknown,
    NotInstalled,
    DetectionFailed,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ResourceState {
    Available,
    Empty,
    Failed,
    NotAvailable,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectionIssue {
    pub source: String,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateShortcutRequest {
    pub website_name: String,
    pub target_url: String,
    pub installation_id: String,
    pub profile_id: String,
    #[serde(default)]
    pub container_id: Option<String>,
    #[serde(default)]
    pub open_as_app: bool,
    pub shortcut_name: String,
    pub icon_background_color: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutRecord {
    pub id: String,
    pub name: String,
    pub website_name: String,
    pub target_url: String,
    pub installation_id: String,
    pub profile_id: String,
    pub profile_path: String,
    pub profile_name: String,
    #[serde(default)]
    pub container_id: Option<String>,
    #[serde(default)]
    pub container_name: Option<String>,
    #[serde(default)]
    pub container_user_context_id: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub taskbar_tab_id: Option<String>,
    #[serde(default)]
    pub open_as_app: bool,
    pub protocol_url: String,
    pub shortcut_path: String,
    pub icon_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_background_color: Option<String>,
    #[serde(default)]
    pub icon_used_fallback: bool,
    #[serde(default)]
    pub icon_warning: Option<String>,
    pub created_at: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutView {
    pub id: String,
    pub name: String,
    pub website_name: String,
    pub target_url: String,
    pub installation_id: String,
    pub profile_id: String,
    pub profile_name: String,
    pub container_id: Option<String>,
    pub container_name: Option<String>,
    pub open_as_app: bool,
    pub protocol_url: String,
    pub shortcut_path: String,
    pub icon_path: String,
    pub icon_preview_data_url: Option<String>,
    pub icon_background_color: Option<String>,
    pub icon_used_fallback: bool,
    pub icon_warning: Option<String>,
    pub status: ShortcutStatus,
    pub created_at: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ShortcutStatus {
    Ready,
    MissingShortcut,
    MissingFirefox,
    MissingProfile,
    MissingContainer,
    MissingTaskbarTab,
    MissingHandler,
    DetectionFailed,
}
