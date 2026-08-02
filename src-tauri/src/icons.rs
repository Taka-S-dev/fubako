//! シェルが持っているファイルの種類アイコンを取り出して data URI にする。
//! 一覧のどの行がどのアプリで開くのかを、名前を読まなくても見分けられるようにする。

use std::path::Path;

/// 拡張子ごとの結果を覚えておく。1つのフォルダには同じ種類が並ぶため、
/// これが無いと同じアイコンを何度も取り出すことになる。
/// 取り出せなかった拡張子も None として覚え、毎回試し直さない
#[cfg(windows)]
static CACHE: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<String, Option<String>>>> =
    std::sync::LazyLock::new(Default::default);

/// アイコンが中身に固有で、拡張子で共有できないもの
#[cfg(windows)]
const PER_FILE: [&str; 4] = ["exe", "lnk", "ico", "url"];

/// 表示用の PNG data URI を返す。取り出せなければ None を返し、
/// 呼び出し側は記号での表示に戻す。一覧の取得自体は失敗させない
/// 同じアイコンを共有できる範囲を表す鍵。フォルダは1種類、拡張子つきは拡張子ごと、
/// 中身に固有のアイコンを持つものだけパスごとに分ける
#[cfg(windows)]
fn cache_key(path: &Path, is_dir: bool, ext: &str) -> String {
    if is_dir {
        "\u{0}dir".to_string()
    } else if PER_FILE.contains(&ext) {
        path.to_string_lossy().to_lowercase()
    } else if ext.is_empty() {
        "\u{0}file".to_string()
    } else {
        ext.to_string()
    }
}

#[cfg(windows)]
pub fn data_uri(path: &Path, is_dir: bool) -> Option<String> {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let per_file = !is_dir && PER_FILE.contains(&ext.as_str());
    let key = cache_key(path, is_dir, &ext);

    if let Some(hit) = CACHE.lock().ok().and_then(|c| c.get(&key).cloned()) {
        return hit;
    }
    let icon = extract(path, is_dir, per_file, &ext);
    if let Ok(mut cache) = CACHE.lock() {
        cache.insert(key, icon.clone());
    }
    icon
}

#[cfg(windows)]
fn extract(path: &Path, is_dir: bool, per_file: bool, ext: &str) -> Option<String> {
    use windows::core::HSTRING;
    use windows::Win32::Storage::FileSystem::{FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_NORMAL};
    use windows::Win32::UI::Shell::{
        SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON, SHGFI_USEFILEATTRIBUTES,
    };
    use windows::Win32::UI::WindowsAndMessaging::DestroyIcon;

    // 実体を見る必要があるのは中身固有のものだけ。それ以外は種類だけ分かればよいので、
    // ディスクへ触らずに済む SHGFI_USEFILEATTRIBUTES で問い合わせる
    let (query, attributes, flags) = if per_file {
        (
            path.to_string_lossy().to_string(),
            FILE_ATTRIBUTE_NORMAL,
            SHGFI_ICON | SHGFI_LARGEICON,
        )
    } else if is_dir {
        (
            "x".to_string(),
            FILE_ATTRIBUTE_DIRECTORY,
            SHGFI_ICON | SHGFI_LARGEICON | SHGFI_USEFILEATTRIBUTES,
        )
    } else {
        (
            if ext.is_empty() { "x".into() } else { format!("x.{ext}") },
            FILE_ATTRIBUTE_NORMAL,
            SHGFI_ICON | SHGFI_LARGEICON | SHGFI_USEFILEATTRIBUTES,
        )
    };

    unsafe {
        let mut info = SHFILEINFOW::default();
        let ok = SHGetFileInfoW(
            &HSTRING::from(query),
            attributes,
            Some(&mut info),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            flags,
        );
        if ok == 0 || info.hIcon.is_invalid() {
            return None;
        }
        let pixels = to_rgba(info.hIcon);
        let _ = DestroyIcon(info.hIcon);
        let (width, height, rgba) = pixels?;
        encode(width, height, &rgba)
    }
}

/// HICON を上下正しい向きの RGBA8 に展開する
#[cfg(windows)]
unsafe fn to_rgba(
    icon: windows::Win32::UI::WindowsAndMessaging::HICON,
) -> Option<(u32, u32, Vec<u8>)> {
    use windows::Win32::Graphics::Gdi::{
        DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC, BITMAP, BITMAPINFO,
        BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
    };
    use windows::Win32::UI::WindowsAndMessaging::{GetIconInfo, ICONINFO};

    let mut ii = ICONINFO::default();
    GetIconInfo(icon, &mut ii).ok()?;
    // ICONINFO のビットマップは呼び出し側が解放する
    let cleanup = |ii: &ICONINFO| {
        if !ii.hbmColor.is_invalid() {
            let _ = DeleteObject(ii.hbmColor.into());
        }
        if !ii.hbmMask.is_invalid() {
            let _ = DeleteObject(ii.hbmMask.into());
        }
    };

    let mut bmp = BITMAP::default();
    let read = GetObjectW(
        ii.hbmColor.into(),
        std::mem::size_of::<BITMAP>() as i32,
        Some(&mut bmp as *mut _ as *mut _),
    );
    if read == 0 || bmp.bmWidth <= 0 || bmp.bmHeight <= 0 {
        cleanup(&ii);
        return None;
    }
    let (w, h) = (bmp.bmWidth as u32, bmp.bmHeight as u32);

    let mut header = BITMAPINFO::default();
    header.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
    header.bmiHeader.biWidth = w as i32;
    // 負の高さで上下逆転を避け、そのまま上から下の順で受け取る
    header.bmiHeader.biHeight = -(h as i32);
    header.bmiHeader.biPlanes = 1;
    header.bmiHeader.biBitCount = 32;
    header.bmiHeader.biCompression = BI_RGB.0;

    let mut buffer = vec![0u8; (w * h * 4) as usize];
    let dc = GetDC(None);
    let lines = GetDIBits(
        dc,
        ii.hbmColor,
        0,
        h,
        Some(buffer.as_mut_ptr() as *mut _),
        &mut header,
        DIB_RGB_COLORS,
    );
    ReleaseDC(None, dc);
    cleanup(&ii);
    if lines == 0 {
        return None;
    }

    // GDI は BGRA で返す。アルファが全て 0 のときは
    // 透過情報を持たない古い形式なので、不透明として扱う
    let opaque = buffer.iter().skip(3).step_by(4).all(|&a| a == 0);
    for px in buffer.chunks_exact_mut(4) {
        px.swap(0, 2);
        if opaque {
            px[3] = 255;
        }
    }
    Some((w, h, buffer))
}

#[cfg(windows)]
fn encode(width: u32, height: u32, rgba: &[u8]) -> Option<String> {
    use base64::Engine as _;

    let mut png = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(rgba).ok()?;
    }
    Some(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&png)
    ))
}

#[cfg(not(windows))]
pub fn data_uri(_path: &Path, _is_dir: bool) -> Option<String> {
    None
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn same_extension_shares_one_icon() {
        let a = cache_key(Path::new(r"C:\work\a\メモ.md"), false, "md");
        let b = cache_key(Path::new(r"C:\work\b\別のメモ.md"), false, "md");
        assert_eq!(a, b);
        assert_ne!(a, cache_key(Path::new(r"C:\work\a\表.xlsx"), false, "xlsx"));
    }

    #[test]
    fn icons_bound_to_the_file_itself_are_not_shared() {
        // 実行ファイルとショートカットはそれぞれ固有のアイコンを持つ
        let a = cache_key(Path::new(r"C:\work\tool.exe"), false, "exe");
        let b = cache_key(Path::new(r"C:\work\other.exe"), false, "exe");
        assert_ne!(a, b);
        // 大文字小文字だけが違うパスは同じものとして扱う
        assert_eq!(a, cache_key(Path::new(r"C:\WORK\Tool.exe"), false, "exe"));
    }

    #[test]
    fn folders_and_extensionless_files_have_their_own_keys() {
        let dir = cache_key(Path::new(r"C:\work\資料"), true, "");
        let file = cache_key(Path::new(r"C:\work\LICENSE"), false, "");
        assert_ne!(dir, file);
        // フォルダは名前によらず1種類にまとまる
        assert_eq!(dir, cache_key(Path::new(r"C:\work\別のフォルダ"), true, ""));
    }
}
