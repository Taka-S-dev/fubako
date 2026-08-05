use crate::model::{AppConfig, Status, Task, TaskMeta};
use chrono::{DateTime, Duration, Local, NaiveDate, TimeZone};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub const META_FILE: &str = ".todo.json";

pub fn read_meta(dir: &Path) -> TaskMeta {
    fs::read_to_string(dir.join(META_FILE))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// 一時ファイルへ書き切ってから rename で置き換えるアトミック書き込み。
/// Rust の `fs::rename` は Windows では MOVEFILE_REPLACE_EXISTING 付きで
/// 呼ばれるため既存ファイルを上書きでき、置き換えの前に消す必要はない。
/// 先に消すと「旧ファイルが無く新ファイルもまだ無い」瞬間ができてしまう
pub fn atomic_write(path: &Path, data: &[u8]) -> Result<(), String> {
    let file_name = path
        .file_name()
        .ok_or("不正なパスです")?
        .to_string_lossy()
        .to_string();
    let tmp = path.with_file_name(format!("{file_name}.tmp"));
    {
        let mut file = fs::File::create(&tmp).map_err(|e| e.to_string())?;
        file.write_all(data).map_err(|e| e.to_string())?;
        // 置き換える前に中身をディスクへ確定させる。これが無いと、
        // rename 後に電源が落ちた場合に空のファイルだけが残り得る
        file.sync_all().map_err(|e| e.to_string())?;
    }
    if let Err(e) = fs::rename(&tmp, path) {
        // 置き換えられなかったときは書きかけを残さない。既存ファイルは無傷
        let _ = fs::remove_file(&tmp);
        return Err(e.to_string());
    }
    Ok(())
}

pub fn write_meta(dir: &Path, meta: &TaskMeta) -> Result<(), String> {
    let json = serde_json::to_string_pretty(meta).map_err(|e| e.to_string())?;
    atomic_write(&dir.join(META_FILE), json.as_bytes())
}

/// フォルダ名から日付プレフィックスを切り出す (例: "20260802_調査" -> ("20260802", "調査"))
pub fn parse_folder_name(folder_name: &str) -> (Option<String>, String) {
    let chars: Vec<char> = folder_name.chars().collect();
    if chars.len() >= 9 && chars[..8].iter().all(|c| c.is_ascii_digit()) && chars[8] == '_' {
        let prefix: String = chars[..8].iter().collect();
        let rest: String = chars[9..].iter().collect();
        let name = if rest.is_empty() { prefix.clone() } else { rest };
        (Some(prefix), name)
    } else {
        (None, folder_name.to_string())
    }
}

fn to_rfc3339(t: SystemTime) -> Option<String> {
    let dt: DateTime<Local> = t.into();
    Some(dt.to_rfc3339())
}

/// フォルダ内で最も新しい更新日時を探す（放置検出用）。
/// 深さ・件数に上限を設けてスキャンコストを抑える。.todo.json は
/// アプリ自身の書き込みで更新されるため作業実績には数えない。
fn newest_mtime(dir: &Path, depth: u32, budget: &mut u32) -> Option<SystemTime> {
    if *budget == 0 {
        return None;
    }
    let mut newest: Option<SystemTime> = None;
    let Ok(entries) = fs::read_dir(dir) else {
        return None;
    };
    for entry in entries.flatten() {
        if *budget == 0 {
            break;
        }
        *budget -= 1;
        let name = entry.file_name().to_string_lossy().to_string();
        if name == META_FILE {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        if let Ok(m) = meta.modified() {
            if newest.is_none_or(|n| m > n) {
                newest = Some(m);
            }
        }
        if meta.is_dir() && depth > 0 {
            if let Some(m) = newest_mtime(&entry.path(), depth - 1, budget) {
                if newest.is_none_or(|n| m > n) {
                    newest = Some(m);
                }
            }
        }
    }
    newest
}

/// 検索用に名前を集めるときに降りる深さ。詳細パネルの一覧と揃えてある
const SEARCH_MAX_DEPTH: u32 = 3;
/// 集める名前の上限。全タスク分をメモリに載せるため、一覧より辛めにする
const SEARCH_MAX_NAMES: usize = 300;

/// フォルダ内の名前を再帰的に集める。サブフォルダの中身は
/// `資料\仕様書.xlsx` の形で入れるので、フォルダ名でもファイル名でも引ける。
/// 戻り値は直下の項目数で、カードに出す「N 項目」に使う
fn collect_names(dir: &Path, prefix: &str, depth: u32, out: &mut Vec<String>) -> usize {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    let mut here = 0;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with(META_FILE) {
            continue;
        }
        here += 1;
        if out.len() >= SEARCH_MAX_NAMES {
            continue;
        }
        let rel = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}\\{name}")
        };
        let is_dir = entry.file_type().is_ok_and(|t| t.is_dir());
        out.push(rel.clone());
        if is_dir && depth + 1 < SEARCH_MAX_DEPTH {
            collect_names(&entry.path(), &rel, depth + 1, out);
        }
    }
    here
}

pub fn scan_task(dir: &Path, archived: bool, stale_days: u32) -> Option<Task> {
    let folder_name = dir.file_name()?.to_string_lossy().to_string();
    let (date_prefix, name) = parse_folder_name(&folder_name);
    let meta = read_meta(dir);

    let mut file_names = Vec::new();
    let file_count = collect_names(dir, "", 0, &mut file_names);

    let mut budget: u32 = 1000;
    let last_activity_time = newest_mtime(dir, 2, &mut budget)
        .or_else(|| fs::metadata(dir).ok().and_then(|m| m.modified().ok()));
    let last_activity = last_activity_time.and_then(to_rfc3339);

    // 保留中は「放置」に数えない。手を止めているのは意図であって放置ではなく、
    // ここを分けないと手の打ちようがない警告が溜まって放置検出そのものが見られなくなる
    let stale = !archived
        && meta.on_hold_since.is_none()
        && last_activity_time.is_some_and(|t| {
            let dt: DateTime<Local> = t.into();
            Local::now() - dt > Duration::days(stale_days as i64)
        });

    // 作成日時: メタ > フォルダ名の日付 > ファイルシステムの作成日時
    let created_at = meta.created_at.clone().or_else(|| {
        date_prefix
            .as_ref()
            .and_then(|p| NaiveDate::parse_from_str(p, "%Y%m%d").ok())
            .and_then(|d| d.and_hms_opt(0, 0, 0))
            .and_then(|ndt| Local.from_local_datetime(&ndt).single())
            .map(|dt| dt.to_rfc3339())
            .or_else(|| {
                fs::metadata(dir)
                    .ok()
                    .and_then(|m| m.created().ok())
                    .and_then(to_rfc3339)
            })
    });

    let status = if archived { Status::Done } else { meta.status };

    let checklist_total = meta.checklist.len();
    let checklist_done = meta.checklist.iter().filter(|i| i.done).count();
    // 進捗率: 見積(分)があれば時間で重み付け。未入力項目は平均見積で補完
    let estimates: Vec<u32> = meta.checklist.iter().filter_map(|i| i.estimate_min).collect();
    let avg_estimate = if estimates.is_empty() {
        1
    } else {
        (estimates.iter().sum::<u32>() / estimates.len() as u32).max(1)
    };
    let weight = |i: &crate::model::ChecklistItem| i.estimate_min.unwrap_or(avg_estimate).max(1) as u64;
    // 手動なのに値が無い状態は「未設定」とみなし、やることがあれば自動計算に戻す
    let manual_value = meta.progress.map(|p| p.min(100));
    let use_manual =
        checklist_total == 0 || (meta.progress_mode == crate::model::ProgressMode::Manual
            && manual_value.is_some());
    let progress = if use_manual {
        manual_value
    } else {
        let total_w: u64 = meta.checklist.iter().map(&weight).sum();
        let done_w: u64 = meta.checklist.iter().filter(|i| i.done).map(&weight).sum();
        Some((done_w * 100 / total_w.max(1)) as u32)
    };
    let remaining_min = if estimates.is_empty() {
        None
    } else {
        Some(
            meta.checklist
                .iter()
                .filter(|i| !i.done)
                .filter_map(|i| i.estimate_min)
                .sum::<u32>(),
        )
    };

    Some(Task {
        path: dir.to_string_lossy().to_string(),
        folder_name,
        name,
        date_prefix,
        status,
        archived,
        tags: meta.tags,
        due: meta.due,
        memo: meta.memo,
        checklist: meta.checklist,
        manual_progress: meta.progress,
        progress_mode: meta.progress_mode,
        progress,
        checklist_done,
        checklist_total,
        remaining_min,
        created_at,
        completed_at: meta.completed_at,
        on_hold_since: meta.on_hold_since,
        last_activity,
        file_count,
        file_names,
        stale,
    })
}

fn subdirs(root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(root) else {
        return vec![];
    };
    entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .is_some_and(|n| !n.to_string_lossy().starts_with('.'))
        })
        .collect()
}

pub fn scan_all(config: &AppConfig) -> Vec<Task> {
    let mut tasks = Vec::new();
    let archive_root = config.archive_root.as_ref().map(PathBuf::from);
    // ディープアーカイブはコールドストレージ: スキャン対象外
    let deep_root = config.deep_archive_root.as_ref().map(PathBuf::from);
    let is_deep = |dir: &PathBuf| deep_root.as_ref() == Some(dir);

    if let Some(work_root) = config.work_root.as_ref().map(PathBuf::from) {
        for dir in subdirs(&work_root) {
            // アーカイブが作業ルート直下にある構成でも二重計上しない
            if (archive_root.as_ref() == Some(&dir)) || is_deep(&dir) {
                continue;
            }
            if let Some(t) = scan_task(&dir, false, config.stale_days) {
                tasks.push(t);
            }
        }
    }

    if let Some(archive_root) = archive_root {
        for dir in subdirs(&archive_root) {
            if is_deep(&dir) {
                continue;
            }
            let name = dir.file_name().map(|n| n.to_string_lossy().to_string());
            let is_year_dir = name
                .as_ref()
                .is_some_and(|n| n.len() == 4 && n.chars().all(|c| c.is_ascii_digit()));
            if is_year_dir {
                for sub in subdirs(&dir) {
                    if is_deep(&sub) {
                        continue;
                    }
                    if let Some(t) = scan_task(&sub, true, config.stale_days) {
                        tasks.push(t);
                    }
                }
            } else if let Some(t) = scan_task(&dir, true, config.stale_days) {
                tasks.push(t);
            }
        }
    }

    tasks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ChecklistItem, ProgressMode};

    #[test]
    fn parse_folder_name_variants() {
        assert_eq!(
            parse_folder_name("20260802_調査"),
            (Some("20260802".into()), "調査".into())
        );
        assert_eq!(parse_folder_name("メモ置き場"), (None, "メモ置き場".into()));
        assert_eq!(
            parse_folder_name("20260802_"),
            (Some("20260802".into()), "20260802".into())
        );
        // 8桁数字+アンダースコアの形式以外はプレフィックス扱いしない
        assert_eq!(parse_folder_name("2026080_x"), (None, "2026080_x".into()));
        assert_eq!(parse_folder_name("20260802x"), (None, "20260802x".into()));
    }

    #[test]
    fn meta_roundtrip_and_atomic_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let meta = TaskMeta {
            status: Status::Doing,
            tags: vec!["調査".into()],
            memo: "めも".into(),
            ..Default::default()
        };
        write_meta(dir.path(), &meta).unwrap();
        // 上書きしても壊れず、一時ファイルも残らない
        write_meta(dir.path(), &meta).unwrap();
        let loaded = read_meta(dir.path());
        assert_eq!(loaded.status, Status::Doing);
        assert_eq!(loaded.tags, vec!["調査".to_string()]);
        assert!(!dir.path().join(".todo.json.tmp").exists());
    }

    #[test]
    fn broken_meta_falls_back_to_default() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(META_FILE), "{ こわれたJSON").unwrap();
        let loaded = read_meta(dir.path());
        assert_eq!(loaded.status, Status::Backlog);
    }

    #[test]
    fn progress_is_weighted_by_estimates() {
        let dir = tempfile::tempdir().unwrap();
        let task_dir = dir.path().join("20260802_調査");
        fs::create_dir(&task_dir).unwrap();
        let meta = TaskMeta {
            status: Status::Doing,
            checklist: vec![
                ChecklistItem { text: "a".into(), done: true, estimate_min: Some(30) },
                ChecklistItem { text: "b".into(), done: true, estimate_min: Some(120) },
                ChecklistItem { text: "c".into(), done: false, estimate_min: Some(90) },
            ],
            ..Default::default()
        };
        write_meta(&task_dir, &meta).unwrap();
        let task = scan_task(&task_dir, false, 14).unwrap();
        // (30+120) / 240 = 62%
        assert_eq!(task.progress, Some(62));
        assert_eq!(task.remaining_min, Some(90));
        assert_eq!(task.checklist_done, 2);
    }

    #[test]
    fn manual_mode_overrides_checklist() {
        let dir = tempfile::tempdir().unwrap();
        let meta = TaskMeta {
            checklist: vec![ChecklistItem { text: "a".into(), done: true, estimate_min: None }],
            progress: Some(30),
            progress_mode: ProgressMode::Manual,
            ..Default::default()
        };
        write_meta(dir.path(), &meta).unwrap();
        let task = scan_task(dir.path(), false, 14).unwrap();
        assert_eq!(task.progress, Some(30));
    }

    #[test]
    fn scan_all_skips_deep_archive_and_reads_year_dirs() {
        let root = tempfile::tempdir().unwrap();
        let work = root.path().join("work");
        let archive = root.path().join("archive");
        let deep = archive.join("deep");
        fs::create_dir_all(work.join("20260802_作業A")).unwrap();
        fs::create_dir_all(archive.join("2026").join("20260601_完了B")).unwrap();
        fs::create_dir_all(deep.join("20250101_古い")).unwrap();

        let config = AppConfig {
            work_root: Some(work.to_string_lossy().into()),
            archive_root: Some(archive.to_string_lossy().into()),
            deep_archive_root: Some(deep.to_string_lossy().into()),
            ..Default::default()
        };
        let tasks = scan_all(&config);
        let names: Vec<&str> = tasks.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"作業A"));
        assert!(names.contains(&"完了B"));
        // ディープアーカイブ配下はスキャンされない
        assert!(!names.iter().any(|n| n.contains("古い")));
        assert!(tasks.iter().find(|t| t.name == "完了B").unwrap().archived);
    }

    #[test]
    fn a_task_on_hold_is_not_counted_as_neglected() {
        let dir = tempfile::tempdir().unwrap();
        let task = dir.path().join("20260101_回答待ち");
        fs::create_dir_all(&task).unwrap();

        // 更新が止まっていれば放置として印が付く
        let idle = scan_task(&task, false, 0).unwrap();
        assert!(idle.stale);

        // 保留にすると印が消える。手を止めているのは意図であって放置ではない
        write_meta(
            &task,
            &TaskMeta {
                on_hold_since: Some(Local::now().to_rfc3339()),
                ..Default::default()
            },
        )
        .unwrap();
        let held = scan_task(&task, false, 0).unwrap();
        assert!(!held.stale);
        assert!(held.on_hold_since.is_some());
    }

    #[test]
    fn manual_without_a_value_falls_back_to_the_checklist() {
        let dir = tempfile::tempdir().unwrap();
        // 手で編集された、または過去のバグで生まれた「手動なのに値が無い」状態
        let meta = TaskMeta {
            checklist: vec![
                ChecklistItem { text: "a".into(), done: true, estimate_min: Some(60) },
                ChecklistItem { text: "b".into(), done: false, estimate_min: Some(60) },
            ],
            progress: None,
            progress_mode: ProgressMode::Manual,
            ..Default::default()
        };
        write_meta(dir.path(), &meta).unwrap();
        let task = scan_task(dir.path(), false, 14).unwrap();
        assert_eq!(task.progress, Some(50));
    }

    #[test]
    fn atomic_write_replaces_existing_content_without_a_gap() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data.json");
        fs::write(&path, b"OLD").unwrap();

        atomic_write(&path, b"NEW").unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "NEW");
        // 置き換えは rename 一発。書きかけの一時ファイルは残らない
        assert!(!dir.path().join("data.json.tmp").exists());
    }

    #[test]
    fn search_names_reach_into_subfolders() {
        let dir = tempfile::tempdir().unwrap();
        let task = dir.path().join("20260802_調査");
        fs::create_dir_all(task.join("資料").join("旧版")).unwrap();
        fs::write(task.join("メモ.md"), b"x").unwrap();
        fs::write(task.join("資料").join("仕様書.xlsx"), b"x").unwrap();
        fs::write(task.join("資料").join("旧版").join("初稿.docx"), b"x").unwrap();
        fs::write(task.join(META_FILE), b"{}").unwrap();

        let mut names = Vec::new();
        let count = collect_names(&task, "", 0, &mut names);
        names.sort();
        assert_eq!(
            names,
            vec![
                "メモ.md",
                "資料",
                r"資料\仕様書.xlsx",
                r"資料\旧版",
                r"資料\旧版\初稿.docx",
            ]
        );
        // カードの「N 項目」は直下だけを数える
        assert_eq!(count, 2);
    }

    #[test]
    fn search_names_stop_at_the_depth_limit() {
        let dir = tempfile::tempdir().unwrap();
        let task = dir.path().join("20260802_調査");
        let deep = task.join("a").join("b").join("c");
        fs::create_dir_all(&deep).unwrap();
        fs::write(deep.join("奥.txt"), b"x").unwrap();

        let mut names = Vec::new();
        collect_names(&task, "", 0, &mut names);
        names.sort();
        assert_eq!(names, vec!["a", r"a\b", r"a\b\c"]);
    }

    #[test]
    fn atomic_write_creates_a_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data.json");
        atomic_write(&path, b"NEW").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "NEW");
    }
}
