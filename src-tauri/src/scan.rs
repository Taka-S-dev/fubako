use crate::model::{AppConfig, Status, Task, TaskMeta};
use chrono::{DateTime, Duration, Local, NaiveDate, TimeZone};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub const META_FILE: &str = ".todo.json";

pub fn read_meta(dir: &Path) -> TaskMeta {
    fs::read_to_string(dir.join(META_FILE))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// 一時ファイルへ書いてから rename で置き換えるアトミック書き込み。
/// 書き込み途中でプロセスが落ちても既存ファイルは壊れない
pub fn atomic_write(path: &Path, data: &[u8]) -> Result<(), String> {
    let file_name = path
        .file_name()
        .ok_or("不正なパスです")?
        .to_string_lossy()
        .to_string();
    let tmp = path.with_file_name(format!("{file_name}.tmp"));
    fs::write(&tmp, data).map_err(|e| e.to_string())?;
    // Windows の rename は上書き不可のため既存ファイルを先に外す
    if path.exists() {
        fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    fs::rename(&tmp, path).map_err(|e| e.to_string())
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

pub fn scan_task(dir: &Path, archived: bool, stale_days: u32) -> Option<Task> {
    let folder_name = dir.file_name()?.to_string_lossy().to_string();
    let (date_prefix, name) = parse_folder_name(&folder_name);
    let meta = read_meta(dir);

    let file_names: Vec<String> = fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.file_name().to_string_lossy().to_string())
                .filter(|n| !n.starts_with(META_FILE))
                .take(300)
                .collect()
        })
        .unwrap_or_default();
    let file_count = file_names.len();

    let mut budget: u32 = 1000;
    let last_activity_time = newest_mtime(dir, 2, &mut budget)
        .or_else(|| fs::metadata(dir).ok().and_then(|m| m.modified().ok()));
    let last_activity = last_activity_time.and_then(to_rfc3339);

    let stale = !archived
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
    let progress = if checklist_total > 0 && meta.progress_mode == crate::model::ProgressMode::Auto
    {
        let total_w: u64 = meta.checklist.iter().map(&weight).sum();
        let done_w: u64 = meta.checklist.iter().filter(|i| i.done).map(&weight).sum();
        Some((done_w * 100 / total_w.max(1)) as u32)
    } else {
        meta.progress.map(|p| p.min(100))
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
}
