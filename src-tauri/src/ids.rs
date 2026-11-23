//! 作業の id を振り、重複を直す。
//!
//! id は .todo.json に入れて一度だけ振り、そのあとは変えない。フォルダの名前を変えても、
//! アーカイブへ移しても .todo.json と一緒に動くので、外の道具はパスではなく id で作業を指せる。

use crate::model::Task;
use crate::scan;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::SystemTime;

pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// 新しい id が要る作業の番号を返す。id が無いものと、ほかと重なったもののうち元ではないほう。
///
/// 重なるのは、ほとんどがエクスプローラーでフォルダごと複製したときで、中の .todo.json が
/// 写ったもの。外の道具が覚えているのは元の作業なので、先にあったほう（フォルダの作成日時が
/// 古いほう）に id を残し、写しのほうを振り直す。作成日時が取れなければパスの順で決め、
/// 読み直すたびに結果が変わらないようにする
fn needs_new_id(tasks: &[Task], created: impl Fn(&Task) -> Option<SystemTime>) -> Vec<usize> {
    let mut fix = Vec::new();
    let mut groups: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, t) in tasks.iter().enumerate() {
        match t.id.as_deref() {
            None => fix.push(i),
            Some(id) => groups.entry(id).or_default().push(i),
        }
    }
    for (_, mut same) in groups {
        if same.len() < 2 {
            continue;
        }
        same.sort_by_cached_key(|&i| {
            let c = created(&tasks[i]);
            (c.is_none(), c, tasks[i].path.clone())
        });
        fix.extend(same.into_iter().skip(1));
    }
    fix.sort_unstable();
    fix
}

/// 一覧を読んだときの指紋のまま変わっていなければ、id を書き込む。
/// 変わっていたら触らず、次に読み直したときにやり直す
fn assign(dir: &Path, id: &str, base_rev: &str) -> Result<String, String> {
    let _guard = crate::ops::meta_lock();
    let (mut meta, rev) = scan::load_meta(dir)?;
    if rev != base_rev {
        return Err(crate::ops::CONFLICT.to_string());
    }
    meta.id = Some(id.to_string());
    scan::write_meta(dir, &meta)
}

/// 一覧の作業に id を振り、重複を直す。書けたものは一覧の値も書き換える。
/// 壊れた記録や新しい版の記録は書き換えないので、それらは id が無いまま一覧に出る。
///
/// 起動時だけでなく読み直すたびに呼ぶ。外の道具が id を付けずに作った作業や、
/// 動かしている間に複製されたフォルダも拾うため。書き込みで見張りがもう一度読み直させるが、
/// そのときには振るものが無いので止まる
pub fn reconcile(tasks: &mut [Task]) {
    let fix = needs_new_id(tasks, |t| {
        fs::metadata(&t.path).and_then(|m| m.created()).ok()
    });
    for i in fix {
        let task = &mut tasks[i];
        let id = new_id();
        if let Ok(rev) = assign(Path::new(&task.path), &id, &task.meta_rev) {
            task.id = Some(id);
            task.meta_rev = rev;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn task_at(dir: &Path) -> Task {
        scan::scan_task(dir, false, 30).unwrap()
    }

    fn task_with_meta(root: &Path, name: &str, json: &str) -> Task {
        let dir = root.join(name);
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join(scan::META_FILE), json).unwrap();
        task_at(&dir)
    }

    #[test]
    fn tasks_without_id_get_one() {
        let root = tempfile::tempdir().unwrap();
        let a = task_with_meta(root.path(), "a", r#"{"status":"doing","memo":"m"}"#);
        // .todo.json が無いフォルダにも振る
        let b_dir = root.path().join("b");
        fs::create_dir(&b_dir).unwrap();
        let mut tasks = vec![a, task_at(&b_dir)];
        reconcile(&mut tasks);

        let ida = tasks[0].id.clone().unwrap();
        assert!(uuid::Uuid::parse_str(&ida).is_ok());
        assert!(tasks[1].id.is_some());
        assert_ne!(tasks[0].id, tasks[1].id);
        // ほかの項目は残り、一覧の指紋は書いたあとのものになる
        let (meta, rev) = scan::load_meta(Path::new(&tasks[0].path)).unwrap();
        assert_eq!(meta.memo, "m");
        assert_eq!(meta.id.as_deref(), Some(ida.as_str()));
        assert_eq!(tasks[0].meta_rev, rev);
    }

    #[test]
    fn existing_id_is_kept() {
        let root = tempfile::tempdir().unwrap();
        let mut tasks = vec![task_with_meta(root.path(), "a", r#"{"id":"keep-me"}"#)];
        let before = tasks[0].meta_rev.clone();
        reconcile(&mut tasks);
        assert_eq!(tasks[0].id.as_deref(), Some("keep-me"));
        // 振るものが無ければ書き込まない
        assert_eq!(tasks[0].meta_rev, before);
    }

    #[test]
    fn copied_folder_gets_a_new_id_and_the_original_keeps_its_own() {
        let root = tempfile::tempdir().unwrap();
        let tasks = vec![
            task_with_meta(root.path(), "original", r#"{"id":"same"}"#),
            task_with_meta(root.path(), "copy", r#"{"id":"same"}"#),
        ];
        let t0 = SystemTime::UNIX_EPOCH;
        let created = |t: &Task| {
            Some(if t.path.ends_with("original") {
                t0
            } else {
                t0 + Duration::from_secs(60)
            })
        };
        assert_eq!(needs_new_id(&tasks, created), vec![1]);
    }

    #[test]
    fn duplicate_without_dates_is_decided_by_path() {
        let root = tempfile::tempdir().unwrap();
        let tasks = vec![
            task_with_meta(root.path(), "b", r#"{"id":"same"}"#),
            task_with_meta(root.path(), "a", r#"{"id":"same"}"#),
        ];
        // "a" が残り、"b"（番号 0）が振り直される
        assert_eq!(needs_new_id(&tasks, |_| None), vec![0]);
    }

    #[test]
    fn empty_id_counts_as_missing() {
        let root = tempfile::tempdir().unwrap();
        let mut tasks = vec![task_with_meta(root.path(), "a", r#"{"id":"  "}"#)];
        reconcile(&mut tasks);
        assert!(tasks[0]
            .id
            .as_deref()
            .is_some_and(|id| !id.trim().is_empty()));
    }

    #[test]
    fn broken_meta_is_left_without_id() {
        let root = tempfile::tempdir().unwrap();
        let mut tasks = vec![task_with_meta(root.path(), "a", "{ 途中")];
        reconcile(&mut tasks);
        assert!(tasks[0].id.is_none());
        let raw = fs::read_to_string(Path::new(&tasks[0].path).join(scan::META_FILE)).unwrap();
        assert_eq!(raw, "{ 途中");
    }

    /// 一覧を読んだあとにほかで書き換えられていたら、その変更を消さない
    #[test]
    fn changed_since_listing_is_not_overwritten() {
        let root = tempfile::tempdir().unwrap();
        let mut tasks = vec![task_with_meta(root.path(), "a", r#"{"memo":"old"}"#)];
        let dir = Path::new(&tasks[0].path).to_path_buf();
        fs::write(dir.join(scan::META_FILE), r#"{"memo":"new"}"#).unwrap();
        reconcile(&mut tasks);
        assert!(tasks[0].id.is_none());
        let (meta, _) = scan::load_meta(&dir).unwrap();
        assert_eq!(meta.memo, "new");
        assert!(meta.id.is_none());
    }
}
