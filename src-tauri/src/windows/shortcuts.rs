use crate::errors::AppError;
use std::{fs, path::Path};
use uuid::Uuid;

pub struct ShortcutTarget<'a> {
    pub executable: &'a Path,
    pub arguments: &'a str,
    pub working_directory: &'a Path,
    pub icon_path: &'a Path,
    pub description: &'a str,
}

#[cfg(windows)]
pub fn create_shortcut(
    destination: &Path,
    target: &ShortcutTarget<'_>,
    replace_existing: bool,
) -> Result<(), AppError> {
    use windows::{
        Win32::{
            Storage::FileSystem::{MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW},
            System::Com::{
                CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
                CoUninitialize, IPersistFile,
            },
            UI::Shell::{IShellLinkW, ShellLink},
        },
        core::{Interface, PCWSTR},
    };

    if destination.exists() && !replace_existing {
        return Err(AppError::new(
            "shortcut_exists",
            "A shortcut with this name already exists. Choose another name.",
            Some(destination.display().to_string()),
        ));
    }

    let parent = destination.parent().ok_or_else(|| {
        AppError::new(
            "invalid_destination",
            "The shortcut destination is not valid.",
            None,
        )
    })?;
    fs::create_dir_all(parent)
        .map_err(|error| AppError::io("The Start menu folder could not be created.", error))?;

    let temp_path = parent.join(format!(".fsm-{}.tmp.lnk", Uuid::new_v4()));
    let temp_wide = wide_path(&temp_path);
    let destination_wide = wide_path(destination);

    let result = (|| {
        let initialized = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.ok();
        if let Err(error) = initialized {
            return Err(AppError::new(
                "shortcut_com_unavailable",
                "Windows could not initialize shortcut support.",
                Some(error.to_string()),
            ));
        }

        let operation = (|| {
            let link: IShellLinkW =
                unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER) }.map_err(
                    |error| {
                        AppError::new(
                            "shortcut_create_failed",
                            "Windows could not create the shortcut.",
                            Some(error.to_string()),
                        )
                    },
                )?;

            let executable = wide_path(target.executable);
            let arguments = wide_string(target.arguments);
            let working_directory = wide_path(target.working_directory);
            let icon_path = wide_path(target.icon_path);
            let description = wide_string(target.description);

            unsafe {
                link.SetPath(PCWSTR(executable.as_ptr()))
                    .and_then(|_| link.SetArguments(PCWSTR(arguments.as_ptr())))
                    .and_then(|_| link.SetWorkingDirectory(PCWSTR(working_directory.as_ptr())))
                    .and_then(|_| link.SetIconLocation(PCWSTR(icon_path.as_ptr()), 0))
                    .and_then(|_| link.SetDescription(PCWSTR(description.as_ptr())))
            }
            .map_err(|error| {
                AppError::new(
                    "shortcut_configure_failed",
                    "Windows could not configure the shortcut.",
                    Some(error.to_string()),
                )
            })?;

            let persist_file: IPersistFile = link.cast().map_err(|error| {
                AppError::new(
                    "shortcut_save_failed",
                    "Windows could not save the shortcut.",
                    Some(error.to_string()),
                )
            })?;
            unsafe { persist_file.Save(PCWSTR(temp_wide.as_ptr()), true) }.map_err(|error| {
                AppError::new(
                    "shortcut_save_failed",
                    "Windows could not save the shortcut.",
                    Some(error.to_string()),
                )
            })?;

            Ok(())
        })();

        unsafe { CoUninitialize() };
        operation
    })();

    if let Err(error) = result {
        let _ = fs::remove_file(&temp_path);
        return Err(error);
    }

    let flags = if replace_existing {
        MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH
    } else {
        MOVEFILE_WRITE_THROUGH
    };
    let moved = unsafe {
        MoveFileExW(
            PCWSTR(temp_wide.as_ptr()),
            PCWSTR(destination_wide.as_ptr()),
            flags,
        )
    };
    if let Err(error) = moved {
        let _ = fs::remove_file(&temp_path);
        return Err(AppError::new(
            "shortcut_save_failed",
            "The Windows shortcut could not be placed in the Start menu.",
            Some(error.to_string()),
        ));
    }

    Ok(())
}

#[cfg(not(windows))]
pub fn create_shortcut(
    _destination: &Path,
    _target: &ShortcutTarget<'_>,
    _replace_existing: bool,
) -> Result<(), AppError> {
    Err(AppError::new(
        "unsupported_platform",
        "Windows Start menu shortcuts can only be created on Windows.",
        None,
    ))
}

fn wide_string(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn wide_path(path: &Path) -> Vec<u16> {
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
