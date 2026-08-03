use crate::model::AppConfig;
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

/// 設定の置き場所は `%APPDATA%\Fubako\config.json`。
/// `app_config_dir()` は逆ドメイン形式の識別子をそのままフォルダ名にするため、
/// 製品名でフォルダが並ぶ Windows の %APPDATA% では浮いてしまう。
/// フォルダ名は productName から取り、設定と表記がずれないようにする
fn config_path(app: &AppHandle) -> Option<PathBuf> {
    let dir = app.path().config_dir().ok()?;
    Some(dir.join(&app.package_info().name).join("config.json"))
}

pub fn load(app: &AppHandle) -> AppConfig {
    let Some(path) = config_path(app) else {
        return AppConfig::default();
    };
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(app: &AppHandle, config: &AppConfig) -> Result<(), String> {
    let path = config_path(app).ok_or("設定ディレクトリを取得できません")?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    crate::scan::atomic_write(&path, json.as_bytes())
}
