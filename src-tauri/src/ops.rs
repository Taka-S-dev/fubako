use crate::model::{AppConfig, ChecklistItem, FolderEntry, ProgressMode, Status, Task, TaskMeta};
use crate::scan::{self, META_FILE};
use crate::{config, watch, AppState};
use chrono::Local;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_autostart::ManagerExt as _;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};
use tauri_plugin_opener::OpenerExt as _;

/// set_task_meta のペイロード。フロントエンドから編集可能なフィールドだけを持つ
#[derive(Debug, serde::Deserialize)]
pub struct MetaPatch {
    pub status: Status,
    pub tags: Vec<String>,
    pub due: Option<String>,
    pub memo: String,
    pub checklist: Vec<ChecklistItem>,
    pub manual_progress: Option<u32>,
    pub progress_mode: ProgressMode,
}

fn sanitize_name(name: &str) -> String {
    name.trim()
        .chars()
        .map(|c| match c {
            '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c => c,
        })
        .collect()
}

/// フォルダを移動する。同一ボリュームは rename、別ボリュームは
/// 「一時フォルダへコピー → rename で確定 → 元を削除」の順で行い、
/// 途中で失敗しても元のフォルダが無傷で残るようにする。
fn move_dir(src: &Path, dest: &Path) -> Result<(), String> {
    let src_c = fs::canonicalize(src).map_err(|e| format!("移動元の解決に失敗: {e}"))?;
    // 再帰移動ガード: 移動先が移動元の内側だとコピーが自己増殖して破損する
    let parent = dest.parent().ok_or("移動先が不正です")?;
    let parent_c =
        fs::canonicalize(parent).map_err(|_| "移動先の親フォルダがありません".to_string())?;
    if parent_c.starts_with(&src_c) {
        return Err("移動先が移動元フォルダの内側にあるため中止しました".to_string());
    }
    if fs::rename(&src_c, dest).is_ok() {
        return Ok(());
    }
    let tmp = dest.with_file_name(format!(".fubako-moving-{}", std::process::id()));
    if tmp.exists() {
        let _ = fs::remove_dir_all(&tmp);
    }
    if let Err(e) = copy_dir_recursive(&src_c, &tmp) {
        let _ = fs::remove_dir_all(&tmp);
        return Err(format!(
            "コピーに失敗したため中止しました（元のフォルダは変更されていません）: {e}"
        ));
    }
    if let Err(e) = fs::rename(&tmp, dest) {
        let _ = fs::remove_dir_all(&tmp);
        return Err(format!(
            "移動先の確定に失敗しました（元のフォルダは変更されていません）: {e}"
        ));
    }
    fs::remove_dir_all(&src_c).map_err(|e| {
        format!("移動は完了しましたが、移動元の削除に失敗しました。手動で削除してください: {e}")
    })
}

fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<(), String> {
    fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(src).map_err(|e| e.to_string())?.flatten() {
        let ty = entry.file_type().map_err(|e| e.to_string())?;
        let to = dest.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &to)?;
        } else {
            fs::copy(entry.path(), &to).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn unique_dest(parent: &Path, folder_name: &str) -> PathBuf {
    let mut dest = parent.join(folder_name);
    let mut n = 2;
    while dest.exists() {
        dest = parent.join(format!("{folder_name}_{n}"));
        n += 1;
    }
    dest
}

fn work_root(config: &AppConfig) -> Result<PathBuf, String> {
    config
        .work_root
        .as_ref()
        .map(PathBuf::from)
        .ok_or_else(|| "作業ディレクトリが未設定です".to_string())
}

fn archive_root(config: &AppConfig) -> Result<PathBuf, String> {
    config
        .archive_root
        .as_ref()
        .map(PathBuf::from)
        .ok_or_else(|| "アーカイブディレクトリが未設定です".to_string())
}

/// タスク操作の対象パスが設定済みルートの配下にあることを確認し、
/// 正規化済みパスを返す。canonicalize によって `..` やジャンクション・
/// シンボリックリンクを解決した実体で判定するため、文字列偽装が効かない
fn ensure_managed(path: &Path, config: &AppConfig) -> Result<PathBuf, String> {
    let canon = fs::canonicalize(path).map_err(|_| "フォルダが見つかりません".to_string())?;
    for root in [&config.work_root, &config.archive_root]
        .iter()
        .filter_map(|r| r.as_ref())
    {
        if let Ok(root_c) = fs::canonicalize(root) {
            if canon.starts_with(&root_c) && canon != root_c {
                return Ok(canon);
            }
        }
    }
    Err("管理対象外のパスです".to_string())
}

/// ルート設定の妥当性検証。同一・入れ子の危険な組み合わせを弾く
fn validate_roots(config: &AppConfig) -> Result<(), String> {
    let canon = |p: &Option<String>| -> Option<PathBuf> {
        p.as_ref().and_then(|s| fs::canonicalize(s).ok())
    };
    let work = canon(&config.work_root);
    let archive = canon(&config.archive_root);
    let deep = canon(&config.deep_archive_root);
    if let (Some(w), Some(a)) = (&work, &archive) {
        if w == a {
            return Err("作業ディレクトリとアーカイブ先が同一です".to_string());
        }
        if w.starts_with(a) {
            return Err("作業ディレクトリをアーカイブ先の内側には置けません".to_string());
        }
    }
    if let Some(d) = &deep {
        if work.as_ref() == Some(d) || archive.as_ref() == Some(d) {
            return Err("ディープアーカイブ先が他のルートと同一です".to_string());
        }
        if let Some(w) = &work {
            if w.starts_with(d) {
                return Err("作業ディレクトリをディープアーカイブ先の内側には置けません".to_string());
            }
        }
    }
    Ok(())
}

/// 隠し設定 display_name をウィンドウタイトルとトレイに反映する
pub fn apply_display_name(app: &AppHandle, config: &AppConfig) {
    if let Some(name) = &config.display_name {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.set_title(name);
        }
        if let Some(tray) = app.tray_by_id("main-tray") {
            let tip = if name.is_empty() { None } else { Some(name.as_str()) };
            let _ = tray.set_tooltip(tip);
        }
    }
}

pub fn register_hotkey(app: &AppHandle, hotkey: &str) -> Result<(), String> {
    let gs = app.global_shortcut();
    gs.unregister_all().map_err(|e| e.to_string())?;
    if hotkey.trim().is_empty() {
        return Ok(());
    }
    let shortcut: Shortcut = hotkey
        .parse()
        .map_err(|_| format!("ホットキーの形式が不正です: {hotkey}"))?;
    gs.register(shortcut).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    Ok(state.config.lock().unwrap().clone())
}

#[tauri::command]
pub async fn set_config(
    app: AppHandle,
    state: State<'_, AppState>,
    config: AppConfig,
) -> Result<(), String> {
    validate_roots(&config)?;
    config::save(&app, &config)?;
    apply_display_name(&app, &config);
    let hotkey_result = register_hotkey(&app, &config.hotkey);
    *state.config.lock().unwrap() = config.clone();
    watch::restart(&app, &config);
    hotkey_result
}

#[tauri::command]
pub async fn list_tasks(state: State<'_, AppState>) -> Result<Vec<Task>, String> {
    let config = state.config.lock().unwrap().clone();
    Ok(scan::scan_all(&config))
}

#[tauri::command]
pub async fn create_task(
    state: State<'_, AppState>,
    name: String,
    use_template: bool,
) -> Result<Task, String> {
    let config = state.config.lock().unwrap().clone();
    let root = work_root(&config)?;
    let name = sanitize_name(&name);
    if name.is_empty() {
        return Err("作業名を入力してください".to_string());
    }
    let folder_name = format!("{}_{}", Local::now().format("%Y%m%d"), name);
    let dir = root.join(&folder_name);
    if dir.exists() {
        return Err(format!("{folder_name} は既に存在します"));
    }
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    if use_template {
        for entry in &config.template_files {
            let entry = entry.trim();
            if entry.is_empty() {
                continue;
            }
            let is_dir = entry.ends_with('/') || entry.ends_with('\\');
            let rel = entry.trim_end_matches(['/', '\\']);
            let sanitized: PathBuf = rel
                .split(['/', '\\'])
                .map(sanitize_name)
                .filter(|s| !s.is_empty() && s != "." && s != "..")
                .collect();
            if sanitized.as_os_str().is_empty() {
                continue;
            }
            let target = dir.join(sanitized);
            if is_dir {
                let _ = fs::create_dir_all(&target);
            } else {
                if let Some(parent) = target.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                if !target.exists() {
                    let _ = fs::write(&target, "");
                }
            }
        }
    }

    let meta = TaskMeta {
        status: Status::Doing,
        created_at: Some(Local::now().to_rfc3339()),
        ..Default::default()
    };
    scan::write_meta(&dir, &meta)?;
    scan::scan_task(&dir, false, config.stale_days).ok_or_else(|| "作成結果の取得に失敗".into())
}

#[tauri::command]
pub async fn set_task_meta(
    state: State<'_, AppState>,
    path: String,
    patch: MetaPatch,
) -> Result<(), String> {
    let config = state.config.lock().unwrap().clone();
    let dir = ensure_managed(Path::new(&path), &config)?;
    let mut meta = scan::read_meta(&dir);
    if patch.status != Status::Done {
        meta.status = patch.status;
    }
    meta.tags = patch.tags;
    meta.due = patch.due;
    meta.memo = patch.memo;
    meta.checklist = patch
        .checklist
        .into_iter()
        .filter(|i| !i.text.trim().is_empty())
        .collect();
    meta.progress = patch.manual_progress.map(|p| p.min(100));
    meta.progress_mode = patch.progress_mode;
    if meta.created_at.is_none() {
        meta.created_at = Some(Local::now().to_rfc3339());
    }
    scan::write_meta(&dir, &meta)
}

#[tauri::command]
pub async fn complete_task(state: State<'_, AppState>, path: String) -> Result<String, String> {
    let config = state.config.lock().unwrap().clone();
    let src = ensure_managed(Path::new(&path), &config)?;
    let archive = archive_root(&config)?;
    let year_dir = archive.join(Local::now().format("%Y").to_string());
    fs::create_dir_all(&year_dir).map_err(|e| e.to_string())?;

    let mut meta = scan::read_meta(&src);
    meta.status = Status::Done;
    meta.completed_at = Some(Local::now().to_rfc3339());
    scan::write_meta(&src, &meta)?;

    let folder_name = src
        .file_name()
        .ok_or("フォルダ名を取得できません")?
        .to_string_lossy()
        .to_string();
    let dest = unique_dest(&year_dir, &folder_name);
    move_dir(&src, &dest)?;
    Ok(dest.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn reopen_task(state: State<'_, AppState>, path: String) -> Result<String, String> {
    let config = state.config.lock().unwrap().clone();
    let src = ensure_managed(Path::new(&path), &config)?;
    let root = work_root(&config)?;

    let mut meta = scan::read_meta(&src);
    meta.status = Status::Doing;
    meta.completed_at = None;
    scan::write_meta(&src, &meta)?;

    let folder_name = src
        .file_name()
        .ok_or("フォルダ名を取得できません")?
        .to_string_lossy()
        .to_string();
    let dest = unique_dest(&root, &folder_name);
    move_dir(&src, &dest)?;
    Ok(dest.to_string_lossy().to_string())
}

/// 完了済みタスクをディープアーカイブ（スキャン対象外のコールドストレージ）へ移動する。
/// 通常アーカイブ内での相対パス（年フォルダなど）はそのまま引き継ぐ
#[tauri::command]
pub async fn deep_archive_task(state: State<'_, AppState>, path: String) -> Result<String, String> {
    let config = state.config.lock().unwrap().clone();
    let src = fs::canonicalize(&path).map_err(|_| "フォルダが見つかりません".to_string())?;
    let archive = fs::canonicalize(archive_root(&config)?)
        .map_err(|_| "アーカイブディレクトリが見つかりません".to_string())?;
    let deep = config
        .deep_archive_root
        .as_ref()
        .map(PathBuf::from)
        .ok_or("ディープアーカイブ先が未設定です（設定画面で指定してください）")?;
    if !src.starts_with(&archive) || src == archive {
        return Err("アーカイブ済みのタスクのみ移動できます".to_string());
    }
    let rel = src
        .strip_prefix(&archive)
        .map_err(|_| "相対パスの解決に失敗".to_string())?;
    let dest_parent = match rel.parent() {
        Some(p) if p.as_os_str().is_empty() => deep.clone(),
        Some(p) => deep.join(p),
        None => deep.clone(),
    };
    fs::create_dir_all(&dest_parent).map_err(|e| e.to_string())?;
    let folder_name = src
        .file_name()
        .ok_or("フォルダ名を取得できません")?
        .to_string_lossy()
        .to_string();
    let dest = unique_dest(&dest_parent, &folder_name);
    move_dir(&src, &dest)?;
    Ok(dest.to_string_lossy().to_string())
}

/// 管理外のフォルダを作業ディレクトリへ移動して管理対象にする（エクスプローラーからのドロップ用）
#[tauri::command]
pub async fn import_task(state: State<'_, AppState>, path: String) -> Result<Task, String> {
    let config = state.config.lock().unwrap().clone();
    let src = fs::canonicalize(&path).map_err(|_| "フォルダが見つかりません".to_string())?;
    if !src.is_dir() {
        return Err(format!(
            "フォルダのみ取り込めます: {}",
            src.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or(path)
        ));
    }
    let root = work_root(&config)?;
    for r in [
        &config.work_root,
        &config.archive_root,
        &config.deep_archive_root,
    ]
    .iter()
    .filter_map(|r| r.as_ref())
    {
        let Ok(r_c) = fs::canonicalize(r) else { continue };
        if src.starts_with(&r_c) {
            return Err("既に管理対象のフォルダです".to_string());
        }
        if r_c.starts_with(&src) {
            return Err("管理ルートを含むフォルダは取り込めません".to_string());
        }
    }
    let folder_name = src
        .file_name()
        .ok_or("フォルダ名を取得できません")?
        .to_string_lossy()
        .to_string();

    let mut meta = scan::read_meta(&src);
    meta.status = Status::Doing;
    if meta.created_at.is_none() {
        meta.created_at = Some(Local::now().to_rfc3339());
    }
    scan::write_meta(&src, &meta)?;

    let dest = unique_dest(&root, &folder_name);
    move_dir(&src, &dest)?;
    scan::scan_task(&dest, false, config.stale_days)
        .ok_or_else(|| "取り込み結果の取得に失敗".to_string())
}

#[tauri::command]
pub async fn open_in_explorer(app: AppHandle, path: String) -> Result<(), String> {
    app.opener()
        .open_path(path, None::<&str>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_folder(
    state: State<'_, AppState>,
    path: String,
) -> Result<Vec<FolderEntry>, String> {
    let config = state.config.lock().unwrap().clone();
    let dir = ensure_managed(Path::new(&path), &config)?;
    let mut entries: Vec<FolderEntry> = fs::read_dir(&dir)
        .map_err(|e| e.to_string())?
        .flatten()
        .filter(|e| !e.file_name().to_string_lossy().starts_with(META_FILE))
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            let modified = meta
                .modified()
                .ok()
                .map(|t| chrono::DateTime::<Local>::from(t).to_rfc3339());
            Some(FolderEntry {
                name: e.file_name().to_string_lossy().to_string(),
                is_dir: meta.is_dir(),
                size: if meta.is_dir() { 0 } else { meta.len() },
                modified,
            })
        })
        .collect();
    entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name)));
    Ok(entries)
}

#[tauri::command]
pub async fn get_autostart(app: AppHandle) -> Result<bool, String> {
    app.autolaunch().is_enabled().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_autostart(app: AppHandle, enabled: bool) -> Result<(), String> {
    let autolaunch = app.autolaunch();
    if enabled {
        autolaunch.enable().map_err(|e| e.to_string())
    } else {
        autolaunch.disable().map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_replaces_invalid_chars() {
        assert_eq!(sanitize_name(r#"a\b/c:d*e?f"g<h>i|j"#), "a_b_c_d_e_f_g_h_i_j");
        assert_eq!(sanitize_name("  調査  "), "調査");
    }

    #[test]
    fn unique_dest_appends_suffix() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("x")).unwrap();
        fs::create_dir(dir.path().join("x_2")).unwrap();
        assert_eq!(unique_dest(dir.path(), "x"), dir.path().join("x_3"));
        assert_eq!(unique_dest(dir.path(), "y"), dir.path().join("y"));
    }

    #[test]
    fn move_dir_moves_folder_with_contents() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        fs::create_dir_all(src.join("sub")).unwrap();
        fs::write(src.join("a.txt"), "a").unwrap();
        fs::write(src.join("sub").join("b.txt"), "b").unwrap();
        let dest = dir.path().join("dest");
        move_dir(&src, &dest).unwrap();
        assert!(!src.exists());
        assert_eq!(fs::read_to_string(dest.join("a.txt")).unwrap(), "a");
        assert_eq!(fs::read_to_string(dest.join("sub").join("b.txt")).unwrap(), "b");
    }

    #[test]
    fn move_dir_rejects_dest_inside_src() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        fs::create_dir_all(src.join("inner")).unwrap();
        fs::write(src.join("a.txt"), "a").unwrap();
        let dest = src.join("inner").join("moved");
        let err = move_dir(&src, &dest).unwrap_err();
        assert!(err.contains("内側"), "unexpected error: {err}");
        // 元のフォルダは無傷
        assert!(src.join("a.txt").exists());
    }

    #[test]
    fn ensure_managed_blocks_path_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let work = dir.path().join("work");
        let secret = dir.path().join("secret");
        fs::create_dir_all(work.join("20260802_task")).unwrap();
        fs::create_dir_all(&secret).unwrap();
        let config = AppConfig {
            work_root: Some(work.to_string_lossy().into()),
            ..Default::default()
        };
        // 配下のタスクはOK
        assert!(ensure_managed(&work.join("20260802_task"), &config).is_ok());
        // ルート自身は対象外
        assert!(ensure_managed(&work, &config).is_err());
        // ".." で外へ出るパスは正規化されて拒否される
        let sneaky = work.join("..").join("secret");
        assert!(ensure_managed(&sneaky, &config).is_err());
        // 存在しないパスも拒否
        assert!(ensure_managed(&work.join("なし"), &config).is_err());
    }

    #[test]
    fn validate_roots_rejects_dangerous_combos() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a");
        fs::create_dir_all(&a).unwrap();
        let same = AppConfig {
            work_root: Some(a.to_string_lossy().into()),
            archive_root: Some(a.to_string_lossy().into()),
            ..Default::default()
        };
        assert!(validate_roots(&same).is_err());

        let b = dir.path().join("b");
        fs::create_dir_all(&b).unwrap();
        let ok = AppConfig {
            work_root: Some(a.to_string_lossy().into()),
            archive_root: Some(b.to_string_lossy().into()),
            ..Default::default()
        };
        assert!(validate_roots(&ok).is_ok());
    }
}
