use crate::{errors::AppError, taskbar_tabs::acquire_profile_lock};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{ErrorKind, Write},
    path::Path,
    sync::Mutex,
};

const CSS: &[u8] = include_bytes!("../../userChrome.css");
const STATE: &str = "fsm-app-chrome-backup.json";
const PREF: &str = "toolkit.legacyUserProfileCustomizations.stylesheets";
static MUTEX: Mutex<()> = Mutex::new(());

#[derive(Deserialize, Serialize)]
struct Backup {
    version: u32,
    css: Option<Vec<u8>>,
    user_js: Option<Vec<u8>>,
    prefs: Option<Vec<u8>>,
    installed_user_js: Vec<u8>,
    installed_css: Vec<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    previous_installed_css: Option<Vec<u8>>,
}

impl Backup {
    fn decode(bytes: &[u8]) -> Result<Self, AppError> {
        let backup: Backup = serde_json::from_slice(bytes).map_err(|e| {
            AppError::io(
                "The toolbar backup is invalid; files were left unchanged.",
                e,
            )
        })?;
        if backup.version != 1 {
            return Err(AppError::new(
                "unsupported_backup",
                "This toolbar backup version is unsupported.",
                None,
            ));
        }
        Ok(backup)
    }

    fn save(&self, path: &Path) -> Result<(), AppError> {
        let bytes = serde_json::to_vec(self)
            .map_err(|error| AppError::io("Could not encode the toolbar backup.", error))?;
        replace(path, Some(&bytes))
    }
}

fn read(path: &Path) -> Result<Option<Vec<u8>>, AppError> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(AppError::io("Could not read Firefox app-toolbar files.", e)),
    }
}

// A temporary sibling and rename prevent partially written profile files.
fn replace(path: &Path, bytes: Option<&[u8]>) -> Result<(), AppError> {
    if let Some(bytes) = bytes {
        let temp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
        let result = (|| -> std::io::Result<()> {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)?;
            file.write_all(bytes)?;
            file.sync_all()?;
            fs::rename(&temp, path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        result.map_err(|e| {
            AppError::io(
                "Could not save Firefox app-toolbar files. The backup has been kept.",
                e,
            )
        })
    } else {
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
            Err(e) => Err(AppError::io(
                "Could not restore Firefox app-toolbar files. The backup has been kept.",
                e,
            )),
        }
    }
}

pub fn is_enabled(profile: &Path) -> Result<bool, AppError> {
    Ok(read(&profile.join(STATE))?.is_some())
}

fn pref_line(line: &str) -> bool {
    let line = line.trim_start();
    line.starts_with("user_pref(")
        && (line.contains(&format!("\"{PREF}\"")) || line.contains(&format!("'{PREF}'")))
}

// Firefox copies user.js preferences into prefs.js at startup. Restore only this
// preference there; other settings changed since installation must survive.
fn restored_prefs(current: &[u8], original: Option<&[u8]>) -> Result<Vec<u8>, AppError> {
    let current = std::str::from_utf8(current)
        .map_err(|e| AppError::io("Firefox preferences are not UTF-8.", e))?;
    let original = std::str::from_utf8(original.unwrap_or_default())
        .map_err(|e| AppError::io("The preference backup is not UTF-8.", e))?;
    let mut result = current
        .split_inclusive('\n')
        .filter(|line| !pref_line(line))
        .collect::<String>();
    for line in original.lines().filter(|line| pref_line(line)) {
        if !result.is_empty() && !result.ends_with('\n') {
            result.push('\n');
        }
        result.push_str(line);
        result.push('\n');
    }
    Ok(result.into_bytes())
}

// CSS strings must escape quotes, backslashes and control characters. Names
// come from Firefox's registry and must never be interpolated as CSS syntax.
fn css_string(value: &str) -> String {
    let mut quoted = String::from("\"");
    for ch in value.chars() {
        match ch {
            '\\' | '"' => {
                quoted.push('\\');
                quoted.push(ch);
            }
            ch if ch.is_control() => quoted.push_str(&format!("\\{:x} ", ch as u32)),
            ch => quoted.push(ch),
        }
    }
    quoted.push('"');
    quoted
}

fn bundled_css(profile: &Path, original: Option<&[u8]>) -> Result<Vec<u8>, AppError> {
    let mut result = original.unwrap_or_default().to_vec();
    result.extend_from_slice(b"\n");
    result.extend_from_slice(CSS);
    if let Some(bytes) = read(&profile.join("taskbartabs/taskbartabs.json"))? {
        let registry: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| {
            AppError::io(
                "Firefox's app names could not be read. The stylesheet was left unchanged.",
                e,
            )
        })?;
        if let Some(apps) = registry.get("taskbarTabs").and_then(|v| v.as_array()) {
            result.extend_from_slice(
                b"\n/* Registered app names (generated by Firefox Shortcut Manager). */\n",
            );
            for app in apps {
                let Some(id) = app.get("id").and_then(|v| v.as_str()) else {
                    continue;
                };
                if uuid::Uuid::parse_str(id).is_err() {
                    continue;
                }
                let Some(name) = app
                    .get("name")
                    .and_then(|v| v.as_str())
                    .filter(|name| !name.trim().is_empty())
                else {
                    continue;
                };
                result.extend_from_slice(
                    format!(
                        ":root[taskbartab={}]:not([popup-window]) {{ --fsm-app-name: {}; }}\n",
                        css_string(id),
                        css_string(name)
                    )
                    .as_bytes(),
                );
            }
        }
    }
    Ok(result)
}

pub fn set_enabled(profile: &Path, enabled: bool) -> Result<(), AppError> {
    let _guard = MUTEX
        .lock()
        .map_err(|e| AppError::io("App-toolbar settings are busy.", e))?;
    let _lock = acquire_profile_lock(profile)?;
    let state = profile.join(STATE);
    let css = profile.join("chrome/userChrome.css");
    let user_js = profile.join("user.js");
    let prefs = profile.join("prefs.js");
    let saved = read(&state)?;
    if enabled {
        if saved.is_some() {
            return Ok(());
        }
        let original_css = read(&css)?;
        let installed_css = bundled_css(profile, original_css.as_deref())?;
        let original_user_js = read(&user_js)?;
        let mut installed_user_js = original_user_js.clone().unwrap_or_default();
        installed_user_js.extend_from_slice(
            format!(
                "\n// Firefox Shortcut Manager: compact app windows\nuser_pref(\"{PREF}\", true);\n"
            )
            .as_bytes(),
        );
        let backup = Backup {
            version: 1,
            css: original_css,
            user_js: original_user_js,
            prefs: read(&prefs)?,
            installed_user_js,
            installed_css,
            previous_installed_css: None,
        };
        fs::create_dir_all(profile.join("chrome"))
            .map_err(|e| AppError::io("Could not create Firefox's chrome folder.", e))?;
        // Persist the originals BEFORE touching either file. If interrupted, disable
        // can recover even when only one of the two replacements completed.
        backup.save(&state)?;
        replace(&css, Some(&backup.installed_css))?;
        replace(&user_js, Some(&backup.installed_user_js))?;
    } else if let Some(saved) = saved {
        let backup = Backup::decode(&saved)?;
        // Do not discard edits made by the user after installation. Accept the
        // original too, so interrupted installs/restores can be retried safely.
        for (path, original, installed) in [
            (&css, &backup.css, &backup.installed_css),
            (&user_js, &backup.user_js, &backup.installed_user_js),
        ] {
            let current = read(path)?;
            let interrupted_update = path == &css
                && current.as_deref().is_some()
                && current.as_deref() == backup.previous_installed_css.as_deref();
            if current != *original
                && current.as_deref() != Some(installed.as_slice())
                && !interrupted_update
            {
                return Err(AppError::new(
                    "toolbar_files_modified",
                    "A toolbar file was edited after installation. Save your edits elsewhere and restore the installed version before disabling. The original backup is safe.",
                    Some(path.display().to_string()),
                ));
            }
        }
        let current_prefs = read(&prefs)?;
        let restored = current_prefs
            .as_deref()
            .map(|current| restored_prefs(current, backup.prefs.as_deref()))
            .transpose()?;
        replace(&css, backup.css.as_deref())?;
        replace(&user_js, backup.user_js.as_deref())?;
        if let Some(restored) = restored {
            replace(&prefs, Some(&restored))?;
        }
        fs::remove_file(state).map_err(|e| AppError::io("Files were restored, but the toolbar backup could not be removed. Retry disabling.", e))?;
    }
    Ok(())
}

/// Only userChrome.css and our backup metadata are touched. Firefox does not
/// write either file, so an update can be staged while it runs; restart applies it.
pub fn update(profile: &Path) -> Result<(), AppError> {
    let _guard = MUTEX
        .lock()
        .map_err(|e| AppError::io("App-toolbar settings are busy.", e))?;
    let state = profile.join(STATE);
    let saved = read(&state)?.ok_or_else(|| {
        AppError::new(
            "toolbar_not_installed",
            "Install the app toolbar before updating it.",
            None,
        )
    })?;
    let mut backup = Backup::decode(&saved)?;
    let css = profile.join("chrome/userChrome.css");
    let current = read(&css)?;
    if current.as_deref() != Some(backup.installed_css.as_slice())
        && !(current.is_some() && current.as_deref() == backup.previous_installed_css.as_deref())
    {
        return Err(AppError::new(
            "toolbar_files_modified",
            "The installed stylesheet has been edited. Save your edits elsewhere before updating. The original backup is safe.",
            Some(css.display().to_string()),
        ));
    }
    let updated = bundled_css(profile, backup.css.as_deref())?;
    if current.as_deref() == Some(updated.as_slice()) && backup.previous_installed_css.is_none() {
        return Ok(());
    }
    // Record both accepted versions before replacing CSS, so an interrupted
    // update remains recoverable by either update or disable. Never re-backup
    // installed CSS as the user's original stylesheet.
    backup.previous_installed_css = current;
    backup.installed_css = updated;
    backup.save(&state)?;
    replace(&css, Some(&backup.installed_css))?;
    backup.previous_installed_css = None;
    backup.save(&state)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn profile() -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("fsm-chrome-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        path
    }
    #[test]
    fn restores_existing_files_and_preserves_new_preferences() {
        let p = profile();
        fs::create_dir(p.join("chrome")).unwrap();
        fs::write(p.join("chrome/userChrome.css"), b"/* original */\r\n").unwrap();
        fs::write(p.join("user.js"), b"// original user preferences\r\n").unwrap();
        let old = format!("user_pref(\"{PREF}\", false);\n");
        fs::write(p.join("prefs.js"), &old).unwrap();
        set_enabled(&p, true).unwrap();
        assert!(
            fs::read(p.join("chrome/userChrome.css"))
                .unwrap()
                .starts_with(b"/* original */\r\n")
        );
        set_enabled(&p, true).unwrap();
        fs::write(
            p.join("prefs.js"),
            format!("user_pref(\"{PREF}\", true);\nuser_pref(\"unrelated\", 42);\n"),
        )
        .unwrap();
        set_enabled(&p, false).unwrap();
        assert_eq!(
            fs::read(p.join("chrome/userChrome.css")).unwrap(),
            b"/* original */\r\n"
        );
        assert_eq!(
            fs::read(p.join("user.js")).unwrap(),
            b"// original user preferences\r\n"
        );
        let prefs = fs::read_to_string(p.join("prefs.js")).unwrap();
        assert!(prefs.contains(&old));
        assert!(prefs.contains("42"));
        assert!(!is_enabled(&p).unwrap());
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn removes_files_that_did_not_exist_and_allows_reinstall() {
        let p = profile();
        for _ in 0..2 {
            set_enabled(&p, true).unwrap();
            set_enabled(&p, false).unwrap();
        }
        assert!(!p.join("chrome/userChrome.css").exists());
        assert!(!p.join("user.js").exists());
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn protects_edits_and_keeps_original_backup() {
        let p = profile();
        set_enabled(&p, true).unwrap();
        fs::write(p.join("chrome/userChrome.css"), "user edits").unwrap();
        assert!(set_enabled(&p, false).is_err());
        assert!(is_enabled(&p).unwrap());
        assert_eq!(
            fs::read_to_string(p.join("chrome/userChrome.css")).unwrap(),
            "user edits"
        );
        let backup: Backup = serde_json::from_slice(&fs::read(p.join(STATE)).unwrap()).unwrap();
        fs::write(p.join("chrome/userChrome.css"), backup.installed_css).unwrap();
        set_enabled(&p, false).unwrap();
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn recovers_an_interrupted_install() {
        let p = profile();
        fs::write(p.join("user.js"), b"// original").unwrap();
        set_enabled(&p, true).unwrap();
        // Simulate termination after the backup but before writing user.js.
        fs::write(p.join("user.js"), b"// original").unwrap();
        set_enabled(&p, false).unwrap();
        assert_eq!(fs::read(p.join("user.js")).unwrap(), b"// original");
        assert!(!p.join("chrome/userChrome.css").exists());
        fs::remove_dir_all(p).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn refuses_changes_while_profile_is_locked() {
        let p = profile();
        let lock = acquire_profile_lock(&p).unwrap();
        assert!(set_enabled(&p, true).is_err());
        assert!(!is_enabled(&p).unwrap());
        drop(lock);
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn removes_the_persisted_preference_if_previously_unset() {
        let p = profile();
        set_enabled(&p, true).unwrap();
        fs::write(
            p.join("prefs.js"),
            format!("user_pref(\"{PREF}\", true);\nuser_pref(\"other\", true);\n"),
        )
        .unwrap();
        set_enabled(&p, false).unwrap();
        let prefs = fs::read_to_string(p.join("prefs.js")).unwrap();
        assert!(!prefs.contains(PREF));
        assert!(prefs.contains("other"));
        fs::remove_dir_all(p).unwrap();
    }
    fn older_install(p: &Path) {
        fs::create_dir_all(p.join("chrome")).unwrap();
        fs::write(p.join("chrome/userChrome.css"), b"/* original */").unwrap();
        set_enabled(p, true).unwrap();
        let state = p.join(STATE);
        let mut backup: Backup = serde_json::from_slice(&fs::read(&state).unwrap()).unwrap();
        backup.installed_css = b"/* older bundled toolbar */".to_vec();
        fs::write(p.join("chrome/userChrome.css"), &backup.installed_css).unwrap();
        fs::write(state, serde_json::to_vec(&backup).unwrap()).unwrap();
    }
    #[test]
    fn updates_older_css_without_losing_original() {
        let p = profile();
        older_install(&p);
        update(&p).unwrap();
        update(&p).unwrap();
        let css = fs::read(p.join("chrome/userChrome.css")).unwrap();
        assert!(css.starts_with(b"/* original */"));
        assert!(css.ends_with(CSS));
        set_enabled(&p, false).unwrap();
        assert_eq!(
            fs::read(p.join("chrome/userChrome.css")).unwrap(),
            b"/* original */"
        );
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn update_does_not_overwrite_user_edits() {
        let p = profile();
        older_install(&p);
        let original_backup = fs::read(p.join(STATE)).unwrap();
        fs::write(p.join("chrome/userChrome.css"), b"edited").unwrap();
        assert!(update(&p).is_err());
        assert_eq!(fs::read(p.join(STATE)).unwrap(), original_backup);
        assert_eq!(
            fs::read(p.join("chrome/userChrome.css")).unwrap(),
            b"edited"
        );
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn interrupted_update_can_be_restored_or_retried() {
        for retry in [false, true] {
            let p = profile();
            older_install(&p);
            let state = p.join(STATE);
            let mut backup: Backup = serde_json::from_slice(&fs::read(&state).unwrap()).unwrap();
            backup.previous_installed_css = Some(backup.installed_css.clone());
            backup.installed_css = b"/* interrupted update target */".to_vec();
            fs::write(state, serde_json::to_vec(&backup).unwrap()).unwrap();
            if retry {
                update(&p).unwrap();
            }
            set_enabled(&p, false).unwrap();
            assert_eq!(
                fs::read(p.join("chrome/userChrome.css")).unwrap(),
                b"/* original */"
            );
            fs::remove_dir_all(p).unwrap();
        }
    }
    #[test]
    fn includes_scoped_registered_app_names_and_escapes_css() {
        let p = profile();
        fs::create_dir(p.join("taskbartabs")).unwrap();
        let id = "aecb3303-eadf-4b55-ba0f-9d1e5fdbd9c3";
        let name = "Barbord \"Work\"\n\\HQ";
        fs::write(
            p.join("taskbartabs/taskbartabs.json"),
            serde_json::to_vec(&serde_json::json!({
                "taskbarTabs": [{"id": id, "name": name}, {"id": "invalid", "name": "Ignored"}]
            }))
            .unwrap(),
        )
        .unwrap();
        set_enabled(&p, true).unwrap();
        let css = fs::read_to_string(p.join("chrome/userChrome.css")).unwrap();
        assert!(css.contains(&format!(":root[taskbartab=\"{id}\"]:not([popup-window])")));
        assert!(css.contains(&format!("--fsm-app-name: {};", css_string(name))));
        assert!(!css.contains("Ignored"));
        set_enabled(&p, false).unwrap();
        assert!(!p.join("chrome/userChrome.css").exists());
        fs::remove_dir_all(p).unwrap();
    }
}
