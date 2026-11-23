//! `fubako://` のリンクを受けて、画面の表示を変える。
//!
//! ほかの道具が Fubako の画面を動かすための入り口。リンクは Web ページやほかのアプリからも
//! 踏めるので、次を守る。
//!
//! - できるのは表示を変えることだけ（作業を選んで詳細を出す、検索欄に言葉を入れる）。
//!   作る・書き換える・ファイルを開く・実行する、はリンクからは受け付けない
//! - 読み取りはここだけで行い、決まった形以外は全部断る。画面には確かめ終わった値だけを渡す
//! - 届いたリンクは最後の1件だけを持つ。続けて踏まれても溜め込まない
//!
//! 受け付ける形:
//!
//! ```text
//! fubako://open?id=<作業の id>     その作業を選んで詳細を出す
//! fubako://search?q=<検索語>       検索欄にその言葉を入れる
//! ```

use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

pub const SCHEME: &str = "fubako";
/// 普通のリンクはこれより十分短い。長いものは読む前に断る
const MAX_URL_LEN: usize = 2048;
const MAX_ID_LEN: usize = 128;
const MAX_QUERY_CHARS: usize = 200;

/// 確かめ終わったリンクの中身。画面はこれだけを受け取る
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum LinkCommand {
    Open { id: String },
    Search { q: String },
}

/// 画面に渡す1件。受け付けなかったリンクも、断ったことを知らせるために渡す
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "result", rename_all = "lowercase")]
pub enum Pending {
    Accepted { command: LinkCommand },
    Rejected { reason: String },
}

/// 届いたリンクのうち、画面がまだ受け取っていない最後の1件
#[derive(Default)]
pub struct PendingLink(pub Mutex<Option<Pending>>);

/// id に使えるのは英数字と - と _ だけ。Fubako が振る UUID はこれに収まり、
/// 区切り文字や制御文字を通さないので、画面でどう使われても形が崩れない
pub(crate) fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_ID_LEN
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

pub fn parse(raw: &str) -> Result<LinkCommand, String> {
    if raw.len() > MAX_URL_LEN {
        return Err("リンクが長すぎます".into());
    }
    let url = url::Url::parse(raw).map_err(|_| "リンクの形が読めません".to_string())?;
    if url.scheme() != SCHEME {
        return Err("Fubako のリンクではありません".into());
    }
    // 使わない部分が付いたリンクは、意図が分からないので受け付けない
    if !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.fragment().is_some()
        || !matches!(url.path(), "" | "/")
    {
        return Err("受け付けない形のリンクです".into());
    }

    let mut params: Vec<(String, String)> = Vec::new();
    for (k, v) in url.query_pairs() {
        if params.iter().any(|(seen, _)| *seen == k) {
            return Err(format!("{k} が2回指定されています"));
        }
        params.push((k.into_owned(), v.into_owned()));
    }
    let only = |key: &str| -> Result<String, String> {
        if let Some((extra, _)) = params.iter().find(|(k, _)| k != key) {
            return Err(format!("知らない指定です: {extra}"));
        }
        params
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
            .ok_or_else(|| format!("{key} の指定がありません"))
    };

    match url.host_str() {
        Some("open") => {
            let id = only("id")?;
            if !valid_id(&id) {
                return Err("id の形が正しくありません".into());
            }
            Ok(LinkCommand::Open { id })
        }
        Some("search") => {
            let q = only("q")?;
            let q = q.trim();
            if q.is_empty() {
                return Err("検索語が空です".into());
            }
            if q.chars().count() > MAX_QUERY_CHARS {
                return Err("検索語が長すぎます".into());
            }
            if q.chars().any(char::is_control) {
                return Err("検索語に使えない文字が入っています".into());
            }
            Ok(LinkCommand::Search { q: q.to_string() })
        }
        _ => Err("知らない操作のリンクです".into()),
    }
}

/// 起動の引数から fubako:// のリンクを取り出す。リンクで起動されると、Windows は
/// 登録したとおり `"exe" "リンク"` で起動するので、引数はリンク1つだけになる。
/// それ以外の形（引数が2つ以上など）は、リンクに見えても受け取らない。
/// リンクの中の `"` で引数を割って別の引数を足す細工も、これで通らなくなる
pub fn link_from_args<I: IntoIterator<Item = String>>(args: I) -> Option<String> {
    let mut rest = args.into_iter().skip(1);
    let first = rest.next()?;
    if rest.next().is_some() {
        return None;
    }
    let prefix = format!("{SCHEME}:");
    first
        .get(..prefix.len())
        .is_some_and(|p| p.eq_ignore_ascii_case(&prefix))
        .then_some(first)
}

/// リンクの登録を設定に合わせる。起動時と、設定を保存したときに呼ぶ。
///
/// - 有効：今の利用者の分（HKCU）に、今動いている exe を登録する。起動のたびに
///   登録し直すので、exe を移しても次の起動で追いつく
/// - 無効：今の利用者の分の登録を消す
///
/// インストーラーはリンクを登録しない（登録するのはここだけ）。開発中のビルドは
/// レジストリに触れない（登録が target 内の exe を指したまま残り、普段使う Fubako への
/// リンクを奪うため）。登録に失敗してもアプリは動かす
pub fn sync_registration(enabled: bool) {
    #[cfg(all(windows, not(debug_assertions)))]
    {
        let _ = if enabled {
            std::env::current_exe()
                .map_err(|e| e.to_string())
                .and_then(|exe| registry::register(registry::KEY, &exe))
        } else {
            registry::unregister(registry::KEY)
        };
    }
    #[cfg(not(all(windows, not(debug_assertions))))]
    let _ = enabled;
}

/// レジストリへの登録。既定値は製品名だけにし、識別子などは書かない
#[cfg(windows)]
#[cfg_attr(debug_assertions, allow(dead_code))]
mod registry {
    use std::path::Path;
    use windows_registry::CURRENT_USER;

    pub const KEY: &str = r"Software\Classes\fubako";

    pub fn register(key: &str, exe: &Path) -> Result<(), String> {
        let exe = exe.display();
        let set = |sub: &str, name: &str, value: &str| {
            let path = if sub.is_empty() {
                key.to_string()
            } else {
                format!(r"{key}\{sub}")
            };
            CURRENT_USER
                .create(&path)
                .and_then(|k| k.set_string(name, value))
                .map_err(|e| e.to_string())
        };
        set("", "", "URL:Fubako")?;
        set("", "URL Protocol", "")?;
        set("DefaultIcon", "", &format!("\"{exe}\",0"))?;
        set(r"shell\open\command", "", &format!("\"{exe}\" \"%1\""))
    }

    pub fn unregister(key: &str) -> Result<(), String> {
        if CURRENT_USER.open(key).is_err() {
            return Ok(());
        }
        CURRENT_USER.remove_tree(key).map_err(|e| e.to_string())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// 本物の fubako の登録には触れず、試すためだけの場所に書いて、最後に必ず消す。
        /// 親のキーを作らないよう、Software の直下に一つだけ作る
        struct TestKey(String);
        impl Drop for TestKey {
            fn drop(&mut self) {
                let _ = CURRENT_USER.remove_tree(&self.0);
            }
        }

        #[test]
        fn registers_with_product_name_and_removes_cleanly() {
            let key = TestKey(format!(r"Software\FubakoTest-{}", uuid::Uuid::new_v4()));
            let exe = Path::new(r"C:\Apps\Fubako\fubako.exe");
            register(&key.0, exe).unwrap();

            let root = CURRENT_USER.open(&key.0).unwrap();
            assert_eq!(root.get_string("").unwrap(), "URL:Fubako");
            assert_eq!(root.get_string("URL Protocol").unwrap(), "");
            let command = CURRENT_USER
                .open(format!(r"{}\shell\open\command", key.0))
                .unwrap();
            assert_eq!(
                command.get_string("").unwrap(),
                r#""C:\Apps\Fubako\fubako.exe" "%1""#
            );

            unregister(&key.0).unwrap();
            assert!(CURRENT_USER.open(&key.0).is_err());
            // 無いものを消しても失敗しない
            unregister(&key.0).unwrap();
        }
    }
}

fn links_enabled(app: &AppHandle) -> bool {
    app.try_state::<crate::AppState>()
        .is_some_and(|s| s.config.lock().unwrap().accept_links)
}

/// 届いたリンクを読み、最後の1件として持ってから画面に知らせる。
/// 画面は知らせを受けて取りに来る。起動の途中で届いて知らせを聞き逃しても、
/// 画面は読み込みの終わりに一度取りに来るので、取りこぼさない
pub fn deliver(app: &AppHandle, raw: &str) {
    // 無効にしている間は、登録が消える前に届いたものも含めて何もしない。
    // 画面も出さない（受け付けない入り口から窓を前に出させない）
    if !links_enabled(app) {
        return;
    }
    let pending = match parse(raw) {
        Ok(command) => Pending::Accepted { command },
        Err(reason) => Pending::Rejected { reason },
    };
    if let Some(state) = app.try_state::<PendingLink>() {
        *state.0.lock().unwrap() = Some(pending);
    }
    crate::tray::show_main(app);
    let _ = app.emit("link-opened", ());
}

#[tauri::command]
pub fn take_pending_link(state: tauri::State<'_, PendingLink>) -> Option<Pending> {
    state.0.lock().unwrap().take()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_open_and_search() {
        assert_eq!(
            parse("fubako://open?id=1a2b3c4d-0000-4000-8000-00000000abcd"),
            Ok(LinkCommand::Open {
                id: "1a2b3c4d-0000-4000-8000-00000000abcd".into()
            })
        );
        assert_eq!(
            parse("fubako://open/?id=abc_DEF-1"),
            Ok(LinkCommand::Open {
                id: "abc_DEF-1".into()
            })
        );
        assert_eq!(
            parse("fubako://search?q=%E8%A6%8B%E7%A9%8D+%23%E8%AA%BF%E6%9F%BB"),
            Ok(LinkCommand::Search {
                q: "見積 #調査".into()
            })
        );
    }

    /// tools/fubako-tasks.ps1 の -SearchInApp が組むリンク（%20 と、& = の符号化）をそのまま受け、
    /// 検索語の中の & や = が別の指定にならない
    #[test]
    fn accepts_links_built_by_the_script() {
        assert_eq!(
            parse("fubako://search?q=%E8%A6%8B%E7%A9%8D%20%23%E8%AA%BF%E6%9F%BB%26x%3D1"),
            Ok(LinkCommand::Search {
                q: "見積 #調査&x=1".into()
            })
        );
    }

    /// 起動の引数は、リンク1つだけのときに限って受け取る
    #[test]
    fn takes_a_link_only_when_it_is_the_sole_argument() {
        let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            link_from_args(args(&["fubako.exe", "fubako://search?q=x"])),
            Some("fubako://search?q=x".into())
        );
        assert_eq!(
            link_from_args(args(&["fubako.exe", "FUBAKO://open?id=a"])),
            Some("FUBAKO://open?id=a".into())
        );
        assert_eq!(link_from_args(args(&["fubako.exe"])), None);
        assert_eq!(link_from_args(args(&["fubako.exe", "--autostart"])), None);
        // リンクの後ろに別の引数が足されていたら、リンクごと受け取らない
        assert_eq!(
            link_from_args(args(&["fubako.exe", "fubako://open?id=a", "--flag"])),
            None
        );
        assert_eq!(
            link_from_args(args(&["fubako.exe", "https://example.com"])),
            None
        );
        assert_eq!(link_from_args(args(&["fubako.exe", "fu"])), None);
    }

    #[test]
    fn rejects_other_schemes_and_actions() {
        for raw in [
            "https://open?id=abc",
            "file:///C:/Windows/System32/calc.exe",
            "fubako://create?name=x",
            "fubako://delete?id=abc",
            "fubako://OPEN?id=abc",
            "fubako:open?id=abc",
            "not a url",
        ] {
            assert!(parse(raw).is_err(), "should reject: {raw}");
        }
    }

    #[test]
    fn rejects_unexpected_parts() {
        for raw in [
            "fubako://open?id=abc&path=C:/x",
            "fubako://open?id=abc&id=def",
            "fubako://open",
            "fubako://open/sub?id=abc",
            "fubako://open?id=abc#frag",
            "fubako://user:pw@open?id=abc",
            "fubako://open:99?id=abc",
            "fubako://search?q=a&id=b",
        ] {
            assert!(parse(raw).is_err(), "should reject: {raw}");
        }
    }

    /// id は区切りや制御文字を通さない。画面や検索の中で別の意味を持たせないため
    #[test]
    fn rejects_ids_with_unsafe_characters() {
        for id in [
            "",
            "..%2F..%2Fx",
            "a%22b",
            "a%20b",
            "a%0Ab",
            "a%3Cscript%3E",
            "%E3%81%82",
        ] {
            assert!(
                parse(&format!("fubako://open?id={id}")).is_err(),
                "should reject id: {id}"
            );
        }
        let long = "a".repeat(MAX_ID_LEN + 1);
        assert!(parse(&format!("fubako://open?id={long}")).is_err());
    }

    #[test]
    fn rejects_bad_queries() {
        assert!(parse("fubako://search?q=").is_err());
        assert!(parse("fubako://search?q=%20%20").is_err());
        assert!(parse("fubako://search?q=a%0Ab").is_err());
        assert!(parse("fubako://search?q=a%00b").is_err());
        let long = "あ".repeat(MAX_QUERY_CHARS + 1);
        assert!(parse(&format!("fubako://search?q={long}")).is_err());
        let huge = format!("fubako://search?q={}", "a".repeat(MAX_URL_LEN));
        assert!(parse(&huge).is_err());
    }
}
