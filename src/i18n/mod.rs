//! Lightweight embedded and hot-pluggable i18n engine for vcd30player.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

const BUILTIN_ZH_CN: &str = include_str!("../../locales/zh-CN.json");
const BUILTIN_EN_US: &str = include_str!("../../locales/en-US.json");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocaleMeta {
    pub locale: String,
    pub name: String,
    #[serde(default)]
    pub author: String,
}

#[derive(Debug, Clone)]
pub struct LocaleData {
    pub meta: LocaleMeta,
    pub entries: HashMap<String, String>,
}

#[derive(Deserialize)]
struct RawLocaleFile {
    _meta: LocaleMeta,
    #[serde(flatten)]
    entries: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppSettings {
    #[serde(default = "default_language")]
    pub language: String,
}

fn default_language() -> String {
    "auto".to_string()
}

pub struct I18nManager {
    locales: HashMap<String, LocaleData>,
    available_locales: Vec<(String, String)>, // (locale_id, display_name)
    selected_language: String,                // "auto" or specific locale_id
    active_locale: String,                    // resolved active locale
    config_path: Option<PathBuf>,
}

impl Default for I18nManager {
    fn default() -> Self {
        Self::new()
    }
}

impl I18nManager {
    pub fn new() -> Self {
        let mut locales = HashMap::new();

        // 1. Load embedded built-in locales
        if let Ok(raw) = serde_json::from_str::<RawLocaleFile>(BUILTIN_ZH_CN) {
            locales.insert(
                raw._meta.locale.clone(),
                LocaleData {
                    meta: raw._meta,
                    entries: raw.entries,
                },
            );
        }
        if let Ok(raw) = serde_json::from_str::<RawLocaleFile>(BUILTIN_EN_US) {
            locales.insert(
                raw._meta.locale.clone(),
                LocaleData {
                    meta: raw._meta,
                    entries: raw.entries,
                },
            );
        }

        // 2. Scan external folders for community translation packs
        scan_external_locales(&mut locales);

        // 3. Build sorted available locales list
        let available_locales = build_available_locales(&locales);

        // 4. Load persisted settings
        let (config_path, settings) = load_settings();

        let selected_language = settings.language;
        let active_locale = resolve_active_locale(&selected_language, &locales);

        Self {
            locales,
            available_locales,
            selected_language,
            active_locale,
            config_path,
        }
    }

    /// Reloads external locales and rebuilds available list
    pub fn reload_locales(&mut self) {
        scan_external_locales(&mut self.locales);
        self.available_locales = build_available_locales(&self.locales);
        self.active_locale = resolve_active_locale(&self.selected_language, &self.locales);
    }

    /// Returns the translated string for the given key, falling back to English or Chinese if missing.
    pub fn t<'a>(&'a self, key: &'a str) -> &'a str {
        if let Some(locale) = self.locales.get(&self.active_locale) {
            if let Some(val) = locale.entries.get(key) {
                return val.as_str();
            }
        }
        // Fallback 1: en-US
        if self.active_locale != "en-US" {
            if let Some(en) = self.locales.get("en-US") {
                if let Some(val) = en.entries.get(key) {
                    return val.as_str();
                }
            }
        }
        // Fallback 2: zh-CN
        if self.active_locale != "zh-CN" {
            if let Some(zh) = self.locales.get("zh-CN") {
                if let Some(val) = zh.entries.get(key) {
                    return val.as_str();
                }
            }
        }
        key
    }

    /// Translates key and replaces placeholders `{0}`, `{1}`, etc.
    pub fn t_fmt(&self, key: &str, args: &[&str]) -> String {
        let mut s = self.t(key).to_string();
        for (i, arg) in args.iter().enumerate() {
            let placeholder = format!("{{{}}}", i);
            s = s.replace(&placeholder, arg);
        }
        s
    }

    pub fn selected_language(&self) -> &str {
        &self.selected_language
    }

    pub fn active_locale(&self) -> &str {
        &self.active_locale
    }

    pub fn available_locales(&self) -> &[(String, String)] {
        &self.available_locales
    }

    /// Returns display name of currently active resolved locale (e.g. "简体中文")
    pub fn resolved_locale_name(&self) -> &str {
        if let Some(loc) = self.locales.get(&self.active_locale) {
            &loc.meta.name
        } else {
            &self.active_locale
        }
    }

    /// Switches the language setting and updates the active locale immediately.
    pub fn set_language(&mut self, lang: String) {
        self.selected_language = lang;
        self.active_locale = resolve_active_locale(&self.selected_language, &self.locales);
        self.save_settings();
    }

    /// Persists settings to disk
    pub fn save_settings(&self) {
        if let Some(ref path) = self.config_path {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let settings = AppSettings {
                language: self.selected_language.clone(),
            };
            if let Ok(json) = serde_json::to_string_pretty(&settings) {
                let _ = std::fs::write(path, json);
            }
        }
    }

    /// Opens the locales folder in the system file manager
    pub fn open_locales_dir(&self) {
        let target_dir = get_locales_search_dirs().into_iter().find(|d| d.exists()).or_else(|| {
            get_app_config_dir().map(|cfg| {
                let p = cfg.join("locales");
                let _ = std::fs::create_dir_all(&p);
                p
            })
        });

        if let Some(dir) = target_dir {
            #[cfg(target_os = "windows")]
            {
                let _ = std::process::Command::new("explorer").arg(&dir).spawn();
            }
            #[cfg(target_os = "macos")]
            {
                let _ = std::process::Command::new("open").arg(&dir).spawn();
            }
            #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
            {
                let _ = std::process::Command::new("xdg-open").arg(&dir).spawn();
            }
        }
    }
}

/// Detects system language tag
pub fn detect_system_locale() -> String {
    if let Some(loc) = sys_locale::get_locale() {
        let lower = loc.to_lowercase();
        if lower.starts_with("zh") {
            return "zh-CN".to_string();
        }
    }
    "en-US".to_string()
}

fn resolve_active_locale(selected: &str, locales: &HashMap<String, LocaleData>) -> String {
    if selected == "auto" {
        let sys = detect_system_locale();
        if locales.contains_key(&sys) {
            sys
        } else {
            "en-US".to_string()
        }
    } else if locales.contains_key(selected) {
        selected.to_string()
    } else {
        "en-US".to_string()
    }
}

fn build_available_locales(locales: &HashMap<String, LocaleData>) -> Vec<(String, String)> {
    let mut list = Vec::new();
    // Builtin priority order: zh-CN, en-US, then alphabetical
    if let Some(zh) = locales.get("zh-CN") {
        list.push(("zh-CN".to_string(), zh.meta.name.clone()));
    }
    if let Some(en) = locales.get("en-US") {
        list.push(("en-US".to_string(), en.meta.name.clone()));
    }
    let mut other: Vec<_> = locales
        .iter()
        .filter(|(k, _)| *k != "zh-CN" && *k != "en-US")
        .map(|(k, v)| (k.clone(), v.meta.name.clone()))
        .collect();
    other.sort_by(|a, b| a.1.cmp(&b.1));
    list.extend(other);
    list
}

fn get_locales_search_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    // 1. Next to current executable: <exe_dir>/locales
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            dirs.push(parent.join("locales"));
        }
    }

    // 2. Working directory: ./locales
    dirs.push(PathBuf::from("locales"));

    // 3. User config directory: <config_dir>/locales
    if let Some(cfg) = get_app_config_dir() {
        dirs.push(cfg.join("locales"));
    }

    dirs
}

fn scan_external_locales(locales: &mut HashMap<String, LocaleData>) {
    for dir in get_locales_search_dirs() {
        if !dir.is_dir() {
            continue;
        }
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file()
                    && path.extension().and_then(|s| s.to_str()) == Some("json")
                {
                    if let Some(file_name) = path.file_name().and_then(|s| s.to_str()) {
                        if file_name.starts_with('_') {
                            continue; // skip _template.json etc.
                        }
                    }
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        if let Ok(raw) = serde_json::from_str::<RawLocaleFile>(&content) {
                            locales.insert(
                                raw._meta.locale.clone(),
                                LocaleData {
                                    meta: raw._meta,
                                    entries: raw.entries,
                                },
                            );
                        }
                    }
                }
            }
        }
    }
}

pub fn get_app_config_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("APPDATA").map(|appdata| PathBuf::from(appdata).join("vcd30player"))
    }
    #[cfg(not(target_os = "windows"))]
    {
        if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
            Some(PathBuf::from(xdg).join("vcd30player"))
        } else if let Some(home) = std::env::var_os("HOME") {
            Some(PathBuf::from(home).join(".config").join("vcd30player"))
        } else {
            None
        }
    }
}

fn load_settings() -> (Option<PathBuf>, AppSettings) {
    let config_path = get_app_config_dir().map(|d| d.join("settings.json"));

    let settings = if let Some(ref path) = config_path {
        if path.is_file() {
            std::fs::read_to_string(path)
                .ok()
                .and_then(|s| serde_json::from_str::<AppSettings>(&s).ok())
                .unwrap_or_else(|| AppSettings {
                    language: default_language(),
                })
        } else {
            AppSettings {
                language: default_language(),
            }
        }
    } else {
        AppSettings {
            language: default_language(),
        }
    };

    (config_path, settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_locales_loaded() {
        let i18n = I18nManager::new();
        assert!(i18n.locales.contains_key("zh-CN"));
        assert!(i18n.locales.contains_key("en-US"));
        assert!(i18n.available_locales.len() >= 2);
    }

    #[test]
    fn test_translation_and_fallbacks() {
        let mut i18n = I18nManager::new();

        // Test English
        i18n.set_language("en-US".to_string());
        assert_eq!(i18n.active_locale(), "en-US");
        assert_eq!(i18n.t("controls.play"), "Play (Space)");
        assert_eq!(i18n.t("titlebar.close"), "Close (Alt+F4)");

        // Test Chinese
        i18n.set_language("zh-CN".to_string());
        assert_eq!(i18n.active_locale(), "zh-CN");
        assert_eq!(i18n.t("controls.play"), "播放 (Space)");
        assert_eq!(i18n.t("titlebar.close"), "关闭 (Alt+F4)");

        // Test Unknown key falls back to key itself
        assert_eq!(i18n.t("non_existent_key_12345"), "non_existent_key_12345");
    }

    #[test]
    fn test_placeholder_formatting() {
        let mut i18n = I18nManager::new();
        i18n.set_language("en-US".to_string());
        let formatted = i18n.t_fmt("status.playing_track", &["02", "Track Title"]);
        assert_eq!(formatted, "Now Playing: Track 02 (Track Title)");

        i18n.set_language("zh-CN".to_string());
        let formatted_cn = i18n.t_fmt("status.playing_track", &["02", "歌曲名称"]);
        assert_eq!(formatted_cn, "正在播放: 轨道 02 (歌曲名称)");
    }

    #[test]
    fn test_auto_resolution() {
        let mut i18n = I18nManager::new();
        i18n.set_language("auto".to_string());
        assert_eq!(i18n.selected_language(), "auto");
        // Active locale must be either zh-CN or en-US (or available locale)
        assert!(i18n.active_locale() == "zh-CN" || i18n.active_locale() == "en-US");
    }
}

