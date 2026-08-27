use crate::model::{AppConfig, WindowState};
use tauri::{
    App, AppHandle, LogicalPosition, LogicalSize, Manager, PhysicalPosition, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder,
};

/// 保存しておいた大きさと位置で開く。
/// 位置は、外付けを外した後などで画面の外を指していることがある。
/// そのまま置くと二度と掴めなくなるため、どの画面にも重ならない位置は捨てる
pub fn restore(window: &WebviewWindow, state: &WindowState) {
    let _ = window.set_size(LogicalSize::new(state.width as f64, state.height as f64));
    if is_on_some_monitor(window, state) {
        let _ = window.set_position(LogicalPosition::new(state.x as f64, state.y as f64));
    } else {
        let _ = window.center();
    }
    if state.maximized {
        let _ = window.maximize();
    }
}

/// 左上の角がいずれかのモニタの内側にあるか
fn is_on_some_monitor(window: &WebviewWindow, state: &WindowState) -> bool {
    let scale = window.scale_factor().unwrap_or(1.0);
    let point = PhysicalPosition::new(
        (state.x as f64 * scale) as i32,
        (state.y as f64 * scale) as i32,
    );
    let Ok(monitors) = window.available_monitors() else {
        return false;
    };
    monitors.iter().any(|m| {
        let p = m.position();
        let size = m.size();
        point.x >= p.x
            && point.y >= p.y
            && point.x < p.x + size.width as i32
            && point.y < p.y + size.height as i32
    })
}

/// 今の大きさと位置を読み取る。最小化中は値が意味を持たないので覚えない
pub fn current(window: &WebviewWindow) -> Option<WindowState> {
    if window.is_minimized().unwrap_or(false) {
        return None;
    }
    let scale = window.scale_factor().ok()?;
    let size = window.inner_size().ok()?.to_logical::<f64>(scale);
    let position = window.outer_position().ok()?.to_logical::<f64>(scale);
    Some(WindowState {
        x: position.x as i32,
        y: position.y as i32,
        width: size.width as u32,
        height: size.height as u32,
        maximized: window.is_maximized().unwrap_or(false),
    })
}

/// 終了時に一度だけ書き出す。動かすたびに保存すると、ドラッグ中ずっと
/// 設定ファイルを書き換えることになる
pub fn save(app: &AppHandle, config: &mut AppConfig) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let Some(state) = current(&window) else {
        return;
    };
    if config.window == Some(state) {
        return;
    }
    config.window = Some(state);
    let _ = crate::config::save(app, config);
}

/// ウィンドウを組み立てる。tauri.conf.json に書かず Rust 側で作るのは、
/// WebView の作業フォルダを指定するため。既定では
/// `%LOCALAPPDATA%\<バンドル識別子>\EBWebView` が作られ、逆ドメイン形式の
/// 名前が利用者のディスクに残る。設定の置き場所を製品名にした判断と揃える
pub fn create(app: &App) -> tauri::Result<WebviewWindow> {
    let data_dir = app
        .path()
        .local_data_dir()
        .map(|d| d.join(&app.package_info().name))?;
    WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
        .title(&app.package_info().name)
        .inner_size(1180.0, 780.0)
        .min_inner_size(340.0, 300.0)
        .data_directory(data_dir)
        .build()
}
