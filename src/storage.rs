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

// --- profiles ---------------------------------------------------------------

pub fn parse_profiles(json: &str) -> Result<Vec<UserProfile>, serde_json::Error> {
    let mut file: UserProfileFile = serde_json::from_str(json.trim_start_matches('\u{feff}'))?;
    sort_profiles(&mut file.profiles);
    Ok(file.profiles)
}

pub fn read_profiles_from(dir: &Path) -> Vec<UserProfile> {
    std::fs::read_to_string(dir.join(USER_PROFILE_FILE))
        .ok()
        .and_then(|s| parse_profiles(&s).ok())
        .unwrap_or_default()
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
    let text = std::fs::read_to_string(path).ok()?;
    let mut s = parse_app_setting(&text).ok()?;
    s.general_setting.use_application_data_path = is_roaming;
    Some(s)
}

/// Write profiles + settings to the location selected by the settings, removing stale
/// copies from the other location when the data path changed.
pub fn move_data(setting: &ApplicationSetting, profiles: &[UserProfile]) {
    let roaming = setting.general_setting.use_application_data_path;
    let _ = write_profiles_to(&data_dir(roaming), profiles);
    let _ = write_app_setting_to(&data_dir(roaming), setting);
    let old = data_dir(!roaming);
    let _ = std::fs::remove_file(old.join(USER_PROFILE_FILE));
    let _ = std::fs::remove_file(old.join(APP_SETTING_FILE));
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
        std::fs::write(dir.join(USER_PROFILE_FILE), "{ not json").unwrap();
        assert!(read_profiles_from(&dir).is_empty());
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
    fn bom_is_tolerated() {
        let json = format!(
            "\u{feff}{}",
            app_setting_to_json(&ApplicationSetting::default())
        );
        assert!(parse_app_setting(&json).is_ok());
    }
}
