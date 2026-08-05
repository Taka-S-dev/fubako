use crate::model::{
    AppConfig, ChecklistItem, FolderEntry, FolderListing, ProgressMode, Status, Task, TaskMeta,
};
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
    /// 保留にするかどうか。開始時刻は受け取らず、切り替えた側で打つ
    pub on_hold: bool,
}

/// 別ボリューム間の移動を示す OS エラーコード
/// (Windows: ERROR_NOT_SAME_DEVICE, Unix: EXDEV)
const CROSS_VOLUME_ERRORS: [i32; 2] = [17, 18];

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
    match fs::rename(&src_c, dest) {
        Ok(()) => return Ok(()),
        // 別ボリュームのときだけコピーで移動する。ファイルを開いている等の理由で
        // 失敗したままコピーに進むと、削除できずフォルダが二重に残る
        Err(e) if e.raw_os_error().is_some_and(|c| CROSS_VOLUME_ERRORS.contains(&c)) => {}
        Err(e) => {
            return Err(format!(
                "フォルダを移動できませんでした。中のファイルを開いていないか確認してください: {e}"
            ))
        }
    }
    // 自分が作ったものだけを片付けられるよう、既存と衝突しない名前を使う
    let tmp = unique_dest(parent, ".fubako-moving");
    if let Err(e) = copy_dir_recursive(&src_c, &tmp) {
        return Err(discard_copy(
            &tmp,
            format!("コピーに失敗したため中止しました（元のフォルダは変更されていません）: {e}"),
        ));
    }
    if let Err(e) = fs::rename(&tmp, dest) {
        return Err(discard_copy(
            &tmp,
            format!("移動先の確定に失敗しました（元のフォルダは変更されていません）: {e}"),
        ));
    }
    fs::remove_dir_all(&src_c).map_err(|e| {
        format!(
            "移動先へのコピーは完了しましたが、移動元を削除できませんでした。\
             同じフォルダが2箇所にあるため、移動元を手動で削除してください: {e}"
        )
    })
}

/// 中断したコピーを破棄する。移動元にはまだ触れていないためコピー側に固有の
/// データはなく、捨てて構わない。捨てられなかったときだけ残骸の場所を伝える
fn discard_copy(tmp: &Path, message: String) -> String {
    match fs::remove_dir_all(tmp) {
        Ok(()) => message,
        Err(_) => format!(
            "{message}（作業用フォルダ {} が残っています。手動で削除してください）",
            tmp.display()
        ),
    }
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

/// 保存自体は成功したが伝えるべきことがある場合に、その文言を返す。
/// ホットキーを他のアプリに取られていても、他の設定まで保存できないのは困る
#[tauri::command]
pub async fn set_config(
    app: AppHandle,
    state: State<'_, AppState>,
    config: AppConfig,
) -> Result<Option<String>, String> {
    validate_roots(&config)?;
    config::save(&app, &config)?;
    apply_display_name(&app, &config);
    let warning = register_hotkey(&app, &config.hotkey)
        .err()
        .map(|e| format!("設定は保存しましたが、ホットキーを登録できませんでした: {e}"));
    *state.config.lock().unwrap() = config.clone();
    watch::restart(&app, &config);
    Ok(warning)
}

/// 起動時の警告を取り出す。画面が出てから一度だけ伝えたいので、読んだら消す
#[tauri::command]
pub async fn take_startup_warning(state: State<'_, AppState>) -> Result<Option<String>, String> {
    Ok(state.startup_warning.lock().unwrap().take())
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
    // 保留の開始時刻はここで打つ。既に保留なら打ち直さず、いつからかを保つ
    meta.on_hold_since = match (patch.on_hold, meta.on_hold_since.take()) {
        (true, Some(since)) => Some(since),
        (true, None) => Some(Local::now().to_rfc3339()),
        (false, _) => None,
    };
    if meta.created_at.is_none() {
        meta.created_at = Some(Local::now().to_rfc3339());
    }
    scan::write_meta(&dir, &meta)
}

/// フォルダ名の日付プレフィックスを維持したまま作業名部分だけ差し替える
fn rename_folder(src: &Path, name: &str, current: &str) -> Result<PathBuf, String> {
    let name = sanitize_name(name);
    if name.is_empty() {
        return Err("作業名を入力してください".to_string());
    }
    let parent = src.parent().ok_or("親フォルダを取得できません")?;
    let folder_name = match scan::parse_folder_name(current).0 {
        Some(prefix) => format!("{prefix}_{name}"),
        None => name,
    };
    if folder_name == current {
        return Ok(src.to_path_buf());
    }
    let dest = parent.join(&folder_name);
    // Windows は大文字小文字を区別しないため、綴り違いだけの変更は衝突扱いにしない
    let case_only = folder_name.to_lowercase() == current.to_lowercase();
    if !case_only && dest.exists() {
        return Err(format!("{folder_name} は既に存在します"));
    }
    fs::rename(src, &dest).map_err(|e| {
        format!("名前を変更できませんでした。フォルダや中のファイルを開いていないか確認してください: {e}")
    })?;
    Ok(dest)
}

/// タスクフォルダ自体の名前を変更する。日付プレフィックスは維持し、
/// 表示名とフォルダ名がずれないよう別名は持たせない
#[tauri::command]
pub async fn rename_task(
    state: State<'_, AppState>,
    path: String,
    name: String,
) -> Result<String, String> {
    let config = state.config.lock().unwrap().clone();
    let src = ensure_managed(Path::new(&path), &config)?;
    let current = src
        .file_name()
        .ok_or("フォルダ名を取得できません")?
        .to_string_lossy()
        .to_string();
    let dest = rename_folder(&src, &name, &current)?;
    let new_name = dest.file_name().ok_or("フォルダ名を取得できません")?;
    // canonicalize は Windows で `\\?\` 付きの表記を返す。一覧側のパスは
    // 正規化前の表記なので、呼び出し元と同じ形に戻さないと選択が外れる
    let plain = Path::new(&path)
        .parent()
        .map(|p| p.join(new_name))
        .unwrap_or_else(|| dest.clone());
    Ok(plain.to_string_lossy().to_string())
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

/// 取り込めるフォルダかを判定する。
/// ディープアーカイブ配下だけは例外で、スキャン対象外＝アプリからは見えていないため、
/// 取り込みが「退避したフォルダを作業へ戻す」手段になる
fn import_guard(src: &Path, config: &AppConfig) -> Result<(), String> {
    for r in [&config.work_root, &config.archive_root]
        .iter()
        .filter_map(|r| r.as_ref())
    {
        let Ok(r_c) = fs::canonicalize(r) else { continue };
        if src.starts_with(&r_c) {
            return Err("既に管理対象のフォルダです".to_string());
        }
    }
    // ルート自身（やそれを含むフォルダ）はどれも取り込めない。丸ごと自分の中へ移すことになる
    for r in [
        &config.work_root,
        &config.archive_root,
        &config.deep_archive_root,
    ]
    .iter()
    .filter_map(|r| r.as_ref())
    {
        let Ok(r_c) = fs::canonicalize(r) else { continue };
        if r_c.starts_with(src) {
            return Err("管理ルートを含むフォルダは取り込めません".to_string());
        }
    }
    Ok(())
}

/// 管理外のフォルダを作業ディレクトリへ移動して管理対象にする（エクスプローラーからのドロップ用）。
/// ディープアーカイブへ退避したフォルダを作業へ戻すのも、この経路で行う
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
    import_guard(&src, &config)?;
    let folder_name = src
        .file_name()
        .ok_or("フォルダ名を取得できません")?
        .to_string_lossy()
        .to_string();

    let mut meta = scan::read_meta(&src);
    meta.status = Status::Doing;
    // 退避したものを戻す場合は完了日が残っている。作業中なのに完了日を持つ状態にしない
    meta.completed_at = None;
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
    // 同じフォルダの窓が積み上がらないよう、開いているものがあればそれを使う
    if crate::explorer::focus_existing_window(Path::new(&path)) {
        return Ok(());
    }
    app.opener()
        .open_path(path, None::<&str>)
        .map_err(|e| e.to_string())
}

/// フォルダの中身の項目を、拡張子の関連付けに従って開く。
/// エディタを内蔵せず OS の既定アプリに委ねるので、`.md` はユーザーが
/// 設定したエディタで開く。ディレクトリはエクスプローラーで開く
#[tauri::command]
pub async fn open_entry(
    state: State<'_, AppState>,
    app: AppHandle,
    path: String,
) -> Result<(), String> {
    let config = state.config.lock().unwrap().clone();
    let target = Path::new(&path);
    ensure_managed(target, &config)?;
    if target.is_dir() && crate::explorer::focus_existing_window(target) {
        return Ok(());
    }
    // 正規化したパスは Windows で `\\?\` が付き関連付けを引けないため、
    // 検証だけ正規化した実体で行い、開くのは受け取ったパスのまま
    app.opener()
        .open_path(path, None::<&str>)
        .map_err(|e| e.to_string())
}

/// 一覧を作るときに降りる深さ。作業フォルダは資料置き場なので、
/// これ以上深いものは中身を見るよりエクスプローラーで開くほうが早い
const LIST_MAX_DEPTH: u32 = 3;
/// 一覧の件数上限。リポジトリや書き出しフォルダを置かれても固まらないようにする
const LIST_MAX_ENTRIES: usize = 500;

/// エクスプローラーの既定表示に合わせ、隠しファイルとシステムファイルは出さない。
/// `.git` のような作業に関係のないフォルダで件数上限を使い切らないためでもある
#[cfg(windows)]
fn is_hidden(meta: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const HIDDEN: u32 = 0x2;
    const SYSTEM: u32 = 0x4;
    meta.file_attributes() & (HIDDEN | SYSTEM) != 0
}

#[cfg(not(windows))]
fn is_hidden(_meta: &fs::Metadata) -> bool {
    false
}

/// フォルダの中身を再帰的に集める。相対パスのまま平らに並べるので、
/// 展開状態を持たずにサブフォルダの中身まで見せられる
fn collect_entries(dir: &Path, prefix: &str, depth: u32, listing: &mut FolderListing) {
    let Ok(read) = fs::read_dir(dir) else { return };
    for entry in read.flatten() {
        if listing.entries.len() >= LIST_MAX_ENTRIES {
            listing.count_capped = true;
            return;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with(META_FILE) {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        if is_hidden(&meta) {
            continue;
        }
        let rel = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}\\{name}")
        };
        let is_dir = meta.is_dir();
        listing.entries.push(FolderEntry {
            is_dir,
            size: if is_dir { 0 } else { meta.len() },
            modified: meta
                .modified()
                .ok()
                .map(|t| chrono::DateTime::<Local>::from(t).to_rfc3339()),
            icon: crate::icons::data_uri(&entry.path(), is_dir),
            rel: rel.clone(),
        });
        if is_dir {
            if depth + 1 < LIST_MAX_DEPTH {
                collect_entries(&entry.path(), &rel, depth + 1, listing);
            } else {
                listing.deeper_omitted = true;
            }
        }
    }
}

fn build_listing(dir: &Path) -> FolderListing {
    let mut listing = FolderListing {
        entries: Vec::new(),
        deeper_omitted: false,
        count_capped: false,
    };
    collect_entries(dir, "", 0, &mut listing);
    // 区切りを NUL に置き換えて並べると、フォルダとその中身が離れずにまとまる。
    // そのまま比較すると `資料2` が `資料\...` より前に割り込んでしまう
    listing
        .entries
        .sort_by_cached_key(|e| e.rel.replace('\\', "\u{0}"));
    listing
}

#[tauri::command]
pub async fn list_folder(
    state: State<'_, AppState>,
    path: String,
) -> Result<FolderListing, String> {
    let config = state.config.lock().unwrap().clone();
    let dir = ensure_managed(Path::new(&path), &config)?;
    Ok(build_listing(&dir))
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
    fn import_guard_rejects_folders_the_app_already_shows() {
        let dir = tempfile::tempdir().unwrap();
        let work = dir.path().join("work");
        let archive = dir.path().join("archive");
        fs::create_dir_all(work.join("20260802_task")).unwrap();
        fs::create_dir_all(archive.join("2026").join("20260601_done")).unwrap();
        fs::create_dir_all(dir.path().join("外").join("資料")).unwrap();
        let config = AppConfig {
            work_root: Some(work.to_string_lossy().into()),
            archive_root: Some(archive.to_string_lossy().into()),
            ..Default::default()
        };
        // import_task と同じく、判定前に実体へ解決したパスを渡す
        let canon = |p: PathBuf| fs::canonicalize(p).unwrap();
        // 一覧に並んでいるものを取り込み直す意味はない
        assert!(import_guard(&canon(work.join("20260802_task")), &config).is_err());
        assert!(import_guard(&canon(archive.join("2026").join("20260601_done")), &config).is_err());
        // 管理ルートを内側に含むフォルダは、自分の中へ自分を移すことになる
        assert!(import_guard(&canon(dir.path().to_path_buf()), &config).is_err());
        // 管理外のフォルダは通る
        assert!(import_guard(&canon(dir.path().join("外").join("資料")), &config).is_ok());
    }

    #[test]
    fn import_guard_allows_taking_back_from_deep_archive() {
        let dir = tempfile::tempdir().unwrap();
        let work = dir.path().join("work");
        let archive = dir.path().join("archive");
        let deep = dir.path().join("deep");
        fs::create_dir_all(&work).unwrap();
        fs::create_dir_all(&archive).unwrap();
        fs::create_dir_all(deep.join("2026").join("20260101_古い調査")).unwrap();
        let config = AppConfig {
            work_root: Some(work.to_string_lossy().into()),
            archive_root: Some(archive.to_string_lossy().into()),
            deep_archive_root: Some(deep.to_string_lossy().into()),
            ..Default::default()
        };
        let canon = |p: PathBuf| fs::canonicalize(p).unwrap();
        // スキャン対象外＝アプリから見えないので、取り込みが唯一の戻す手段になる
        assert!(import_guard(&canon(deep.join("2026").join("20260101_古い調査")), &config).is_ok());
        // ただしディープアーカイブのルート自身は戻せない
        assert!(import_guard(&canon(deep.clone()), &config).is_err());
    }

    #[test]
    fn hotkey_accepts_what_the_settings_screen_produces() {
        // 設定画面が組み立てる形式は、どれもそのまま登録できる
        for accel in [
            "Ctrl+Alt+KeyE",
            "Ctrl+Shift+Digit1",
            "Alt+F5",
            "Ctrl+Alt+Space",
            "Ctrl+Alt+ArrowUp",
        ] {
            assert!(accel.parse::<Shortcut>().is_ok(), "登録できない: {accel}");
        }
        // 修飾キーが無くても解釈は通る。単独キーを全体のホットキーにすると
        // どのアプリでもその文字が打てなくなるため、弾くのは設定画面側の責任
        assert!("KeyE".parse::<Shortcut>().is_ok());
        // 解釈できないのは、キーが欠けている場合と綴りが違う場合だけ
        assert!("Ctrl+Alt+".parse::<Shortcut>().is_err());
        assert!("Hoge+KeyE".parse::<Shortcut>().is_err());
    }

    #[test]
    fn listing_keeps_a_folder_next_to_its_contents() {
        let dir = tempfile::tempdir().unwrap();
        let task = dir.path().join("20260802_task");
        fs::create_dir_all(task.join("資料")).unwrap();
        fs::create_dir_all(task.join("資料2")).unwrap();
        fs::write(task.join("資料").join("仕様書.xlsx"), b"x").unwrap();
        fs::write(task.join("資料2").join("控え.txt"), b"x").unwrap();
        fs::write(task.join("メモ.md"), b"x").unwrap();
        fs::write(task.join(META_FILE), b"{}").unwrap();

        let rels: Vec<String> = build_listing(&task)
            .entries
            .into_iter()
            .map(|e| e.rel)
            .collect();
        // `資料2` が `資料\仕様書.xlsx` の前に割り込まないこと
        assert_eq!(
            rels,
            vec![
                "メモ.md",
                r"資料",
                r"資料\仕様書.xlsx",
                r"資料2",
                r"資料2\控え.txt",
            ]
        );
    }

    #[test]
    fn listing_stops_at_the_depth_limit_and_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let task = dir.path().join("20260802_task");
        let deep = task.join("a").join("b").join("c");
        fs::create_dir_all(&deep).unwrap();
        fs::write(deep.join("奥のファイル.txt"), b"x").unwrap();

        let listing = build_listing(&task);
        let rels: Vec<&str> = listing.entries.iter().map(|e| e.rel.as_str()).collect();
        assert_eq!(rels, vec!["a", r"a\b", r"a\b\c"]);
        // 打ち切ったことを伝えないと「これで全部」と読まれてしまう
        assert!(listing.deeper_omitted);
        assert!(!listing.count_capped);
    }

    #[test]
    fn listing_stops_at_the_count_limit_and_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let task = dir.path().join("20260802_task");
        fs::create_dir_all(&task).unwrap();
        for i in 0..(LIST_MAX_ENTRIES + 20) {
            fs::write(task.join(format!("{i:04}.txt")), b"x").unwrap();
        }
        let listing = build_listing(&task);
        assert_eq!(listing.entries.len(), LIST_MAX_ENTRIES);
        assert!(listing.count_capped);
    }

    #[test]
    fn ensure_managed_accepts_files_inside_a_task() {
        let dir = tempfile::tempdir().unwrap();
        let work = dir.path().join("work");
        let task = work.join("20260802_task");
        fs::create_dir_all(&task).unwrap();
        fs::write(task.join("メモ.md"), b"# note").unwrap();
        fs::create_dir_all(task.join("資料")).unwrap();
        fs::write(dir.path().join("outside.md"), b"x").unwrap();
        let config = AppConfig {
            work_root: Some(work.to_string_lossy().into()),
            ..Default::default()
        };
        // タスク内のファイルとサブフォルダはどちらも開ける
        assert!(ensure_managed(&task.join("メモ.md"), &config).is_ok());
        assert!(ensure_managed(&task.join("資料"), &config).is_ok());
        // 管理ルートの外にあるファイルは弾く
        assert!(ensure_managed(&dir.path().join("outside.md"), &config).is_err());
        assert!(ensure_managed(&task.join("..").join("..").join("outside.md"), &config).is_err());
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

    #[test]
    fn rename_keeps_date_prefix_and_sanitizes() {
        let dir = tempfile::tempdir().unwrap();
        let work = dir.path().join("work");
        let src = work.join("20260802_調査");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("a.txt"), "a").unwrap();
        let config = AppConfig {
            work_root: Some(work.to_string_lossy().into()),
            ..Default::default()
        };
        let canon = ensure_managed(&src, &config).unwrap();
        let current = canon.file_name().unwrap().to_string_lossy().to_string();
        let renamed = rename_folder(&canon, "設計/レビュー", &current).unwrap();
        assert_eq!(
            renamed.file_name().unwrap().to_string_lossy(),
            "20260802_設計_レビュー"
        );
        assert!(renamed.join("a.txt").exists());
        assert!(!src.exists());
    }

    #[test]
    fn rename_rejects_existing_name() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("20260802_a");
        let b = dir.path().join("20260802_b");
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&b).unwrap();
        let err = rename_folder(&a, "b", "20260802_a").unwrap_err();
        assert!(err.contains("既に存在"), "unexpected: {err}");
        assert!(a.exists());
    }

    #[test]
    fn rename_without_prefix_replaces_whole_name() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("メモ置き場");
        fs::create_dir_all(&src).unwrap();
        let renamed = rename_folder(&src, "資料置き場", "メモ置き場").unwrap();
        assert_eq!(renamed.file_name().unwrap().to_string_lossy(), "資料置き場");
    }

    // Windows は中のファイルが開かれているとフォルダ名を変更できない。
    // その際に元が壊れたり二重コピーが残ったりしないことを固定する
    #[cfg(windows)]
    #[test]
    fn move_dir_reports_open_files_instead_of_copying() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("open.txt"), "x").unwrap();
        let _held = fs::File::open(src.join("open.txt")).unwrap();

        let dest = dir.path().join("dest");
        let err = move_dir(&src, &dest).unwrap_err();
        assert!(err.contains("開いていないか"), "unexpected: {err}");
        // 元は無傷、移動先も一時フォルダも残らない
        assert_eq!(fs::read_to_string(src.join("open.txt")).unwrap(), "x");
        assert!(!dest.exists());
        let leftovers: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.starts_with(".fubako-moving"))
            .collect();
        assert!(leftovers.is_empty(), "temp folder left behind: {leftovers:?}");
    }

    #[cfg(windows)]
    #[test]
    fn rename_reports_open_files() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("20260802_調査");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("open.txt"), "x").unwrap();
        let _held = fs::File::open(src.join("open.txt")).unwrap();

        let err = rename_folder(&src, "別名", "20260802_調査").unwrap_err();
        assert!(err.contains("開いていないか"), "unexpected: {err}");
        assert!(src.exists());
    }

    #[test]
    fn discard_copy_removes_the_temporary_copy() {
        let dir = tempfile::tempdir().unwrap();
        let tmp = dir.path().join(".fubako-moving");
        fs::create_dir_all(tmp.join("sub")).unwrap();
        fs::write(tmp.join("sub").join("a.txt"), "a").unwrap();

        let message = discard_copy(&tmp, "中止しました".to_string());
        // 片付けに成功したときは残骸の案内を足さない
        assert_eq!(message, "中止しました");
        assert!(!tmp.exists());
    }

    #[test]
    fn move_dir_keeps_unrelated_temporary_folders() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        fs::create_dir_all(&src).unwrap();
        // 別プロセスが残したものを消してしまわないこと
        let stale = dir.path().join(".fubako-moving");
        fs::create_dir_all(&stale).unwrap();
        fs::write(stale.join("keep.txt"), "keep").unwrap();

        move_dir(&src, &dir.path().join("dest")).unwrap();
        assert_eq!(fs::read_to_string(stale.join("keep.txt")).unwrap(), "keep");
    }
}
