//! 期限つきの作業を、Obsidian の Tasks プラグインの書き方で md に書き出す。
//!
//! Vault の中のノートしか読まない道具（Tasks プラグインの一覧や、ノートから期限を拾う道具）に
//! Fubako の期限を渡すための出口。一方通行で、md を直しても Fubako には戻らない。
//! 追記ではなく毎回ファイル全体を作り直すので、期限の変更や完了がそのまま反映され、
//! 書き込みに失敗しても次の書き出しで正しい中身に戻る。

use crate::model::{AppConfig, Status, Task};
use crate::scan;
use chrono::NaiveDate;
use std::fs;
use std::path::{Path, PathBuf};

/// 書き出したファイルの1行目。Fubako が作ったファイルかどうかをこれで見分け、
/// 利用者や他の道具が書いたノートを上書きしない
const MARKER: &str = "<!-- fubako:export v1";

const HEADER: &str = "<!-- fubako:export v1 — Fubako が書き出したファイルです。\
ここを直しても Fubako には戻らず、次の書き出しで上書きされます -->\n";

/// Tasks プラグインが項目の印として読む絵文字。作業名に入っていると、
/// 名前の途中から期限や繰り返しとして読まれてしまうので取り除く
const SIGNIFIERS: &[char] = &[
    '📅', '⏳', '🛫', '➕', '✅', '❌', '🔁', '🔺', '⏫', '🔼', '🔽', '⏬', '🆔', '⛔', '🏁',
];

fn clean_name(name: &str) -> String {
    let kept: String = name.chars().filter(|c| !SIGNIFIERS.contains(c)).collect();
    kept.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 作業名を、Fubako でその作業を開くリンクにする。押すと Fubako が開いて詳細を出す。
/// 名前に `[` `]` `\` が入っているとリンクの形が崩れるので、前に `\` を付けて文字として扱わせる。
/// id が無い、またはリンクに使えない形なら、名前だけを書く
fn linked_name(name: &str, id: Option<&str>) -> String {
    let Some(id) = id.filter(|id| crate::deeplink::valid_id(id)) else {
        return name.to_string();
    };
    let mut text = String::with_capacity(name.len());
    for c in name.chars() {
        if matches!(c, '[' | ']' | '\\') {
            text.push('\\');
        }
        text.push(c);
    }
    format!("[{text}]({}://open?id={id})", crate::deeplink::SCHEME)
}

/// 書き出す中身を作る。期限があり、まだ終わっていない、作業側の作業だけを期限の順に並べる。
/// 印は作業中も未着手も `[ ]` にする。Obsidian の標準の見た目では `[ ]` 以外の印（作業中の
/// `[/]` など）がチェック済みの箱になり、終わっていない作業が終わったように見えてしまう。
///
/// `with_links` が真なら、作業名を Fubako でその作業を開くリンクにする。
/// リンクを受け付ける設定が無効なら押しても開かないので、名前だけを書く
pub fn render(tasks: &[Task], include_on_hold: bool, with_links: bool) -> String {
    let mut rows: Vec<(NaiveDate, String, &Task)> = tasks
        .iter()
        .filter(|t| !t.archived && t.status != Status::Done)
        .filter(|t| include_on_hold || t.on_hold_since.is_none())
        .filter_map(|t| {
            // 読めない期限は、書いても見張る側が読めないので出さない
            let due = NaiveDate::parse_from_str(t.due.as_deref()?, "%Y-%m-%d").ok()?;
            Some((due, clean_name(&t.name), t))
        })
        .filter(|(_, name, _)| !name.is_empty())
        .collect();
    rows.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));

    let mut out = String::from(HEADER);
    for (due, name, t) in rows {
        let hold = if t.on_hold_since.is_some() {
            "（保留中）"
        } else {
            ""
        };
        let name = if with_links {
            linked_name(&name, t.id.as_deref())
        } else {
            name
        };
        out.push_str(&format!("- [ ] {name}{hold} 📅 {due}\n"));
    }
    out
}

fn is_ours_or_empty(content: &str) -> bool {
    content.trim().is_empty() || content.starts_with(MARKER)
}

fn inside(path: &Path, root: &Option<String>) -> bool {
    root.as_ref()
        .and_then(|r| fs::canonicalize(r).ok())
        .is_some_and(|r| path.starts_with(r))
}

/// 書き出し先として使えるか確かめ、使えるならその場所を返す。
/// 設定を保存するときと、書き出すたびの両方で呼ぶ（設定のあとでファイルが差し替わることもある）
pub fn check_target(raw: &str, config: &AppConfig) -> Result<PathBuf, String> {
    let path = PathBuf::from(raw.trim());
    if !path.is_absolute() {
        return Err("期限の書き出し先は、ドライブから始まるパスで指定してください".into());
    }
    if !path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("md"))
    {
        return Err("期限の書き出し先は .md のファイルにしてください".into());
    }
    let file_name = path
        .file_name()
        .ok_or("書き出し先のファイル名がありません")?;
    let parent = path
        .parent()
        .and_then(|p| fs::canonicalize(p).ok())
        .filter(|p| p.is_dir())
        .ok_or("期限の書き出し先のフォルダが見つかりません")?;
    // 作業と一緒に動く場所に置くと、アーカイブやフォルダの移動に巻き込まれる
    if inside(&parent, &config.work_root)
        || inside(&parent, &config.archive_root)
        || inside(&parent, &config.deep_archive_root)
    {
        return Err("期限の書き出し先を、作業ディレクトリやアーカイブ先の中には置けません".into());
    }
    let target = parent.join(file_name);
    if target.is_dir() {
        return Err("期限の書き出し先がフォルダになっています".into());
    }
    if target.exists() {
        let content = fs::read_to_string(&target)
            .map_err(|e| format!("{} を読めません: {e}", target.display()))?;
        if !is_ours_or_empty(&content) {
            return Err(format!(
                "{} は Fubako が書き出したファイルではないため、上書きしません。\
                 新しいファイル名を指定してください",
                target.display()
            ));
        }
    }
    Ok(target)
}

/// 中身が変わったときだけ書く。変わらないのに書くと、Obsidian と見張る側に
/// 無駄な読み直しをさせるうえ、ファイルの更新日時が意味を持たなくなる
fn write_if_changed(path: &Path, content: &str) -> Result<bool, String> {
    if fs::read_to_string(path).is_ok_and(|old| old == content) {
        return Ok(false);
    }
    scan::atomic_write(path, content.as_bytes())?;
    Ok(true)
}

/// 設定どおりに書き出す。書き出し先が空なら何もしない
pub fn run(config: &AppConfig, tasks: &[Task]) -> Result<(), String> {
    let Some(raw) = config.export_md.as_deref().filter(|s| !s.trim().is_empty()) else {
        return Ok(());
    };
    let target = check_target(raw, config)?;
    let content = render(tasks, config.export_include_on_hold, config.accept_links);
    write_if_changed(&target, &content).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(name: &str, status: Status, due: Option<&str>) -> Task {
        let dir = tempfile::tempdir().unwrap();
        let mut t = scan::scan_task(dir.path(), false, 30).unwrap();
        t.name = name.into();
        t.status = status;
        t.due = due.map(Into::into);
        t
    }

    fn body(s: &str) -> Vec<&str> {
        s.lines().skip(1).collect()
    }

    #[test]
    fn lists_open_tasks_with_due_in_due_order() {
        let mut archived = task("アーカイブ済み", Status::Done, Some("2026-11-01"));
        archived.archived = true;
        let tasks = vec![
            task("見積もり作成", Status::Doing, Some("2026-12-03")),
            task("期限なし", Status::Doing, None),
            task("障害調査", Status::Backlog, Some("2026-11-28")),
            task("日付が変", Status::Backlog, Some("来週")),
            archived,
        ];
        let out = render(&tasks, true, true);
        assert!(out.starts_with(MARKER));
        assert_eq!(
            body(&out),
            vec![
                "- [ ] 障害調査 📅 2026-11-28",
                "- [ ] 見積もり作成 📅 2026-12-03",
            ]
        );
    }

    #[test]
    fn on_hold_is_marked_or_left_out() {
        let mut held = task("待ち", Status::Doing, Some("2026-12-01"));
        held.on_hold_since = Some("2026-11-20T10:00:00+09:00".into());
        let tasks = vec![held];
        assert_eq!(
            body(&render(&tasks, true, true)),
            vec!["- [ ] 待ち（保留中） 📅 2026-12-01"]
        );
        assert!(body(&render(&tasks, false, true)).is_empty());
    }

    /// 作業名は Fubako でその作業を開くリンクになる。リンクの形を崩す文字は文字として扱わせる
    #[test]
    fn names_link_back_to_the_task() {
        let mut t = task(r"見積[第2版]\控え", Status::Backlog, Some("2026-12-01"));
        t.id = Some("1a2b3c4d-0000-4000-8000-00000000abcd".into());
        let mut bad = task("id が変", Status::Backlog, Some("2026-12-02"));
        bad.id = Some("a b)(x".into());
        assert_eq!(
            body(&render(&[t, bad], true, true)),
            vec![
                r"- [ ] [見積\[第2版\]\\控え](fubako://open?id=1a2b3c4d-0000-4000-8000-00000000abcd) 📅 2026-12-01",
                "- [ ] id が変 📅 2026-12-02",
            ]
        );
    }

    /// リンクを受け付ける設定が無効なら、押しても開かないリンクは書かない
    #[test]
    fn names_are_plain_when_links_are_off() {
        let mut t = task("見積", Status::Backlog, Some("2026-12-01"));
        t.id = Some("1a2b3c4d".into());
        assert_eq!(
            body(&render(&[t], true, false)),
            vec!["- [ ] 見積 📅 2026-12-01"]
        );
    }

    /// 作業名に Tasks の印が入っていても、期限の読み違いを起こさない
    #[test]
    fn signifiers_in_names_are_removed() {
        let tasks = vec![task("📅 締め ⏫ 対応", Status::Backlog, Some("2026-12-01"))];
        assert_eq!(
            body(&render(&tasks, true, true)),
            vec!["- [ ] 締め 対応 📅 2026-12-01"]
        );
    }

    #[test]
    fn foreign_notes_are_never_overwritten() {
        let vault = tempfile::tempdir().unwrap();
        let config = AppConfig::default();
        let note = vault.path().join("ToDo.md");
        fs::write(&note, "- [ ] 自分で書いた行 📅 2026-12-01\n").unwrap();
        let err = check_target(&note.to_string_lossy(), &config).unwrap_err();
        assert!(err.contains("上書きしません"), "unexpected: {err}");

        // 空のノートと、Fubako が書いたものは使える
        let empty = vault.path().join("empty.md");
        fs::write(&empty, "").unwrap();
        assert!(check_target(&empty.to_string_lossy(), &config).is_ok());
        let ours = vault.path().join("ours.md");
        fs::write(&ours, render(&[], true, true)).unwrap();
        assert!(check_target(&ours.to_string_lossy(), &config).is_ok());
    }

    #[test]
    fn bad_targets_are_refused() {
        let base = tempfile::tempdir().unwrap();
        let work = base.path().join("work");
        fs::create_dir(&work).unwrap();
        let config = AppConfig {
            work_root: Some(work.to_string_lossy().into()),
            ..Default::default()
        };
        let p = |rel: &str| base.path().join(rel).to_string_lossy().to_string();
        assert!(check_target(&p("a.txt"), &config).is_err());
        assert!(check_target(&p("missing/a.md"), &config).is_err());
        assert!(check_target(&p("work/a.md"), &config).is_err());
        assert!(check_target("a.md", &config).is_err());
        assert!(check_target(&p("a.md"), &config).is_ok());
    }

    #[test]
    fn writes_only_when_content_changes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("期限.md");
        assert!(write_if_changed(&path, "a").unwrap());
        assert!(!write_if_changed(&path, "a").unwrap());
        assert!(write_if_changed(&path, "b").unwrap());
        assert_eq!(fs::read_to_string(&path).unwrap(), "b");
    }

    #[test]
    fn nothing_happens_without_a_target() {
        let config = AppConfig::default();
        assert!(run(&config, &[]).is_ok());
    }
}
