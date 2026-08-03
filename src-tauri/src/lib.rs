mod config;
mod explorer;
mod icons;
mod model;
mod ops;
mod scan;
mod tray;
mod watch;

use std::sync::Mutex;
use tauri::{Emitter, Manager};

pub struct AppState {
    pub config: Mutex<model::AppConfig>,
    pub watcher: Mutex<watch::WatcherHandle>,
    /// 起動時に伝えられなかった不具合。画面が出てから一度だけ取り出す
    pub startup_warning: Mutex<Option<String>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            tray::show_main(app);
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        tray::show_main(app);
                        let _ = app.emit("focus-search", ());
                    }
                })
                .build(),
        )
        .setup(|app| {
            let cfg = config::load(app.handle());
            // リリースビルドには標準エラー出力が無いため、画面へ渡せるよう持っておく
            let warning = ops::register_hotkey(app.handle(), &cfg.hotkey)
                .err()
                .map(|e| format!("ホットキーを登録できませんでした: {e}"));
            app.manage(AppState {
                config: Mutex::new(cfg.clone()),
                watcher: Mutex::new(None),
                startup_warning: Mutex::new(warning),
            });
            watch::restart(app.handle(), &cfg);
            tray::setup(app)?;
            ops::apply_display_name(app.handle(), &cfg);
            Ok(())
        })
        .on_window_event(|window, event| {
            // 閉じる = トレイへ格納。終了はトレイメニューから
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            ops::get_config,
            ops::set_config,
            ops::list_tasks,
            ops::create_task,
            ops::set_task_meta,
            ops::rename_task,
            ops::complete_task,
            ops::reopen_task,
            ops::import_task,
            ops::deep_archive_task,
            ops::take_startup_warning,
            ops::open_in_explorer,
            ops::open_entry,
            ops::list_folder,
            ops::get_autostart,
            ops::set_autostart,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
