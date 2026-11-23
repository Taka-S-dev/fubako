//! ほかの道具向けのスクリプトを、設定と同じ場所（`%APPDATA%\Fubako\tools\`）に置く。
//!
//! スクリプトはアプリの中に持っていて、起動のたびに書き出す。インストーラーでも
//! exe 1つの持ち運び版でも同じ場所に置かれ、Fubako を調べに来た道具が設定の隣で見つけられる。
//! アプリと一緒に更新されるので、スクリプトが読む .todo.json の形とずれない

use std::fs;
use std::path::Path;

/// 置くスクリプト。名前と中身は tools/ にあるものと同じ
const SCRIPTS: &[(&str, &str)] = &[(
    "fubako-tasks.ps1",
    include_str!("../../tools/fubako-tasks.ps1"),
)];

/// 中身が違うときだけ書く。手で書き換えられていても、アプリの版のものに戻す。
/// 書いたファイルの数を返す
pub fn install(app_dir: &Path) -> Result<usize, String> {
    let dir = app_dir.join("tools");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut written = 0;
    for (name, content) in SCRIPTS {
        let path = dir.join(name);
        if fs::read(&path).is_ok_and(|old| old == content.as_bytes()) {
            continue;
        }
        crate::scan::atomic_write(&path, content.as_bytes())?;
        written += 1;
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installs_once_and_restores_edits() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(install(dir.path()).unwrap(), 1);
        assert_eq!(install(dir.path()).unwrap(), 0);
        let path = dir.path().join("tools").join("fubako-tasks.ps1");
        fs::write(&path, "changed").unwrap();
        assert_eq!(install(dir.path()).unwrap(), 1);
        // Windows PowerShell 5.1 が日本語を読めるよう、BOM ごと同じ中身で置く
        let bytes = fs::read(&path).unwrap();
        assert!(bytes.starts_with(&[0xEF, 0xBB, 0xBF]));
    }
}
