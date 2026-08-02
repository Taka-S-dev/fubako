//! エクスプローラー連携。同じフォルダのウィンドウを何度も開くと同じ窓が積み上がるため、
//! 既に開いているものがあれば新しく開かずに前面へ出す。

use std::path::Path;

/// 比較用にパスを正規化する。`\\?\` 接頭辞と末尾の区切りを外し、小文字化する
fn comparable(path: &str) -> String {
    let trimmed = path.strip_prefix(r"\\?\").unwrap_or(path);
    trimmed.trim_end_matches(['\\', '/']).to_lowercase()
}

/// `file:///C:/dir/%E8%AA%BF%E6%9F%BB` 形式の URL をパス表記に戻す
fn url_to_path(url: &str) -> Option<String> {
    let rest = url.strip_prefix("file:///")?;
    let bytes = rest.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok()?;
            if let Ok(byte) = u8::from_str_radix(hex, 16) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(if bytes[i] == b'/' { b'\\' } else { bytes[i] });
        i += 1;
    }
    String::from_utf8(out).ok()
}

/// 対象フォルダを表示しているエクスプローラーのウィンドウを前面に出す。
/// 見つからない場合や列挙に失敗した場合は false を返し、呼び出し側で通常どおり開く
#[cfg(windows)]
pub fn focus_existing_window(target: &Path) -> bool {
    use windows::core::Interface;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::System::Variant::VARIANT;
    use windows::Win32::UI::Shell::{IShellWindows, IWebBrowser2, ShellWindows};
    use windows::Win32::UI::WindowsAndMessaging::{
        IsIconic, SetForegroundWindow, ShowWindow, SW_RESTORE,
    };

    let target = comparable(&target.to_string_lossy());

    unsafe {
        // 既に初期化済みなら S_FALSE が返る。成功した回数だけ解放する
        let initialized = CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok();

        let found = (|| -> Option<HWND> {
            let shell: IShellWindows = CoCreateInstance(&ShellWindows, None, CLSCTX_ALL).ok()?;
            for index in 0..shell.Count().ok()? {
                let Ok(dispatch) = shell.Item(&VARIANT::from(index)) else {
                    continue;
                };
                let Ok(browser) = dispatch.cast::<IWebBrowser2>() else {
                    continue;
                };
                let Ok(url) = browser.LocationURL() else {
                    continue;
                };
                let Some(path) = url_to_path(&url.to_string()) else {
                    continue;
                };
                if comparable(&path) == target {
                    if let Ok(handle) = browser.HWND() {
                        return Some(HWND(handle.0 as *mut core::ffi::c_void));
                    }
                }
            }
            None
        })();

        if initialized {
            CoUninitialize();
        }

        match found {
            Some(hwnd) => {
                if IsIconic(hwnd).as_bool() {
                    let _ = ShowWindow(hwnd, SW_RESTORE);
                }
                SetForegroundWindow(hwnd).as_bool()
            }
            None => false,
        }
    }
}

#[cfg(not(windows))]
pub fn focus_existing_window(_target: &Path) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_percent_encoded_urls() {
        assert_eq!(
            url_to_path("file:///C:/work/%E8%AA%BF%E6%9F%BB").as_deref(),
            Some(r"C:\work\調査")
        );
        assert_eq!(
            url_to_path("file:///C:/work/plain").as_deref(),
            Some(r"C:\work\plain")
        );
        // フォルダ以外のウィンドウ（コントロールパネル等）は対象外
        assert_eq!(url_to_path("ms-settings:display"), None);
    }

    #[test]
    fn compares_paths_ignoring_prefix_and_case() {
        assert_eq!(comparable(r"\\?\C:\Work\Task\"), comparable(r"c:\work\task"));
        assert_ne!(comparable(r"C:\work\a"), comparable(r"C:\work\b"));
    }
}
