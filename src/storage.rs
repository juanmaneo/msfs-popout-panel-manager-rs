//! Reading and writing `userprofiledata.json` / `appsettingdata.json`.
//!
//! Locations match upstream: `Documents\MSFS Pop Out Panel Manager`, or the roaming
//! `%APPDATA%` equivalent when `UseApplicationDataPath` is set.

use crate::model::{
    AppSettingFile, ApplicationSetting, UserProfile, UserProfileFile, sort_profiles,
};
use std::path::{Path, PathBuf};

pub const FOLDER_NAME: &str = "MSFS Pop Out Panel Manager";
pub const USER_PROFILE_FILE: &str = "userprofiledata.json";
pub const APP_SETTING_FILE: &str = "appsettingdata.json";

pub fn data_dir(roaming: bool) -> PathBuf {
    let base = if roaming {
        dirs::config_dir()
    } else {
        dirs::document_dir()
    }
    .or_else(dirs::home_dir)
    .unwrap_or_else(|| PathBuf::from("."));
    base.join(FOLDER_NAME)
}

pub fn log_dir(roaming: bool) -> PathBuf {
    data_dir(roaming).join("LogFiles")
}

fn write_atomic(path: &Path, content: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, content)?;
    std::fs::rename(&tmp, path).or_else(|_| {
        // rename over an existing file can fail on some file systems; fall back to a plain write
        let r = std::fs::write(path, content);
        let _ = std::fs::remove_file(&tmp);
        r
    })
}

/// Read and parse a data file. A missing file is normal (first run). An unreadable or
/// corrupt one is logged and, so that the next save cannot destroy it, copied to `.bak`.
fn read_data_file<T>(
    path: &Path,
    parse: impl FnOnce(&str) -> Result<T, serde_json::Error>,
) -> Option<T> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(e) => {
            log::error!("Unable to read data file {}: {e}", path.display());
            return None;
        }
    };
    match parse(&text) {
        Ok(value) => Some(value),
        Err(e) => {
            log::error!("Unable to parse data file {}: {e}", path.display());
            let backup = path.with_extension("json.bak");
            if let Err(e) = std::fs::copy(path, &backup) {
                log::error!("Unable to back up {}: {e}", backup.display());
            }
            None
        }
    }
}

// --- profiles ---------------------------------------------------------------

pub fn parse_profiles(json: &str) -> Result<Vec<UserProfile>, serde_json::Error> {
    let mut file: UserProfileFile = serde_json::from_str(json.trim_start_matches('\u{feff}'))?;
    sort_profiles(&mut file.profiles);
    Ok(file.profiles)
}

pub fn read_profiles_from(dir: &Path) -> Vec<UserProfile> {
    read_data_file(&dir.join(USER_PROFILE_FILE), parse_profiles).unwrap_or_default()
}

pub fn profiles_to_json(profiles: &[UserProfile]) -> String {
    let file = UserProfileFile {
        profiles: profiles.to_vec(),
        ..UserProfileFile::default()
    };
    serde_json::to_string_pretty(&file).unwrap_or_default()
}

pub fn write_profiles_to(dir: &Path, profiles: &[UserProfile]) -> std::io::Result<()> {
    write_atomic(&dir.join(USER_PROFILE_FILE), &profiles_to_json(profiles))
}

pub fn read_profiles(roaming: bool) -> Vec<UserProfile> {
    read_profiles_from(&data_dir(roaming))
}

// --- application settings -----------------------------------------------------

pub fn parse_app_setting(json: &str) -> Result<ApplicationSetting, serde_json::Error> {
    let file: AppSettingFile = serde_json::from_str(json.trim_start_matches('\u{feff}'))?;
    let mut s = file.application_setting;
    s.keyboard_shortcut_setting.migrate_legacy();
    Ok(s)
}

pub fn app_setting_to_json(setting: &ApplicationSetting) -> String {
    let file = AppSettingFile {
        application_setting: setting.clone(),
        ..AppSettingFile::default()
    };
    serde_json::to_string_pretty(&file).unwrap_or_default()
}

pub fn write_app_setting_to(dir: &Path, setting: &ApplicationSetting) -> std::io::Result<()> {
    write_atomic(&dir.join(APP_SETTING_FILE), &app_setting_to_json(setting))
}

/// Read the settings file. The roaming copy takes precedence (like upstream);
/// `UseApplicationDataPath` is derived from which location was used.
/// Returns `None` if no readable file exists.
pub fn read_app_setting() -> Option<ApplicationSetting> {
    let roaming = data_dir(true).join(APP_SETTING_FILE);
    let docs = data_dir(false).join(APP_SETTING_FILE);
    let (path, is_roaming) = if roaming.exists() {
        (roaming, true)
    } else {
        (docs, false)
    };
    let mut s = read_data_file(&path, parse_app_setting)?;
    s.general_setting.use_application_data_path = is_roaming;
    Some(s)
}

/// Write profiles + settings to the location selected by the settings, then remove the
/// whole folder of the previous location (upstream deletes it recursively, logs included).
pub fn move_data(setting: &ApplicationSetting, profiles: &[UserProfile]) -> std::io::Result<()> {
    let roaming = setting.general_setting.use_application_data_path;
    move_data_between(&data_dir(!roaming), &data_dir(roaming), setting, profiles)
}

fn move_data_between(
    old: &Path,
    new: &Path,
    setting: &ApplicationSetting,
    profiles: &[UserProfile],
) -> std::io::Result<()> {
    write_profiles_to(new, profiles)?;
    write_app_setting_to(new, setting)?;
    if old != new && old.exists() {
        std::fs::remove_dir_all(old)
            .map_err(|e| std::io::Error::new(e.kind(), format!("{}: {e}", old.display())))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    fn temp_dir() -> PathBuf {
        let d = std::env::temp_dir().join(format!("popm-storage-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn profiles_round_trip_through_disk_and_are_sorted() {
        let dir = temp_dir();
        let mut a = UserProfile::new("zeta");
        a.panel_configs
            .push(PanelConfig::new(PanelType::CustomPopout, "Pfd"));
        let b = UserProfile::new("Alpha");
        write_profiles_to(&dir, &[a.clone(), b.clone()]).unwrap();

        let read = read_profiles_from(&dir);
        assert_eq!(read.len(), 2);
        assert_eq!(read[0].name, "Alpha");
        assert_eq!(read[1].name, "zeta");
        assert_eq!(read[1].panel_configs[0].panel_name, "Pfd");
        assert_eq!(read[1].id, a.id);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn missing_or_corrupt_profile_file_gives_empty_list() {
        let dir = temp_dir();
        assert!(read_profiles_from(&dir).is_empty());
        assert!(!dir.join("userprofiledata.json.bak").exists()); // missing is not corrupt
        std::fs::write(dir.join(USER_PROFILE_FILE), "{ not json").unwrap();
        assert!(read_profiles_from(&dir).is_empty());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn corrupt_file_is_kept_as_backup() {
        let dir = temp_dir();
        std::fs::write(dir.join(USER_PROFILE_FILE), "{ not json").unwrap();
        assert!(read_profiles_from(&dir).is_empty());
        let backup = std::fs::read_to_string(dir.join("userprofiledata.json.bak")).unwrap();
        assert_eq!(backup, "{ not json");
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn settings_round_trip() {
        let dir = temp_dir();
        let mut s = ApplicationSetting::default();
        s.dynamic_lod_setting.targeted_fps = 42;
        s.keyboard_shortcut_setting.pop_out_keyboard_binding = Some("A|B".into());
        write_app_setting_to(&dir, &s).unwrap();
        let text = std::fs::read_to_string(dir.join(APP_SETTING_FILE)).unwrap();
        let back = parse_app_setting(&text).unwrap();
        assert_eq!(back.dynamic_lod_setting.targeted_fps, 42);
        assert_eq!(
            back.keyboard_shortcut_setting
                .pop_out_keyboard_binding
                .as_deref(),
            Some("A|B")
        );
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn moving_data_removes_the_whole_old_folder() {
        let root = temp_dir();
        let (old, new) = (root.join("old"), root.join("new"));
        std::fs::create_dir_all(old.join("LogFiles")).unwrap();
        std::fs::write(old.join("LogFiles").join("error.log"), "x").unwrap();
        std::fs::write(old.join(USER_PROFILE_FILE), "{}").unwrap();

        let mut setting = ApplicationSetting::default();
        setting.dynamic_lod_setting.targeted_fps = 33;
        move_data_between(&old, &new, &setting, &[UserProfile::new("a")]).unwrap();

        assert!(!old.exists());
        assert_eq!(read_profiles_from(&new).len(), 1);
        assert!(new.join(APP_SETTING_FILE).exists());

        // moving onto itself must never delete the data
        move_data_between(&new, &new, &setting, &[UserProfile::new("a")]).unwrap();
        assert_eq!(read_profiles_from(&new).len(), 1);
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn bom_is_tolerated() {
        let json = format!(
            "\u{feff}{}",
            app_setting_to_json(&ApplicationSetting::default())
        );
        assert!(parse_app_setting(&json).is_ok());
    }
}
