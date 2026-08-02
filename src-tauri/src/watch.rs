use crate::model::AppConfig;
use crate::AppState;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

/// 作業ルートとアーカイブルートを監視し、変化があれば "tasks-changed" を
/// フロントエンドへ通知する。設定変更時は作り直す（古い watcher は drop され、
/// 送信側が閉じることで対応するデバウンススレッドも終了する）。
pub fn restart(app: &AppHandle, config: &AppConfig) {
    let state = app.state::<AppState>();
    let mut guard = state.watcher.lock().unwrap();
    *guard = None;

    let roots: Vec<String> = [&config.work_root, &config.archive_root]
        .iter()
        .filter_map(|r| r.as_ref())
        .filter(|r| Path::new(r).is_dir())
        .cloned()
        .collect();
    if roots.is_empty() {
        return;
    }

    let (tx, rx) = mpsc::channel::<()>();
    let watcher = notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
        if res.is_ok() {
            let _ = tx.send(());
        }
    });
    let Ok(mut watcher) = watcher else { return };

    for root in &roots {
        let _ = watcher.watch(Path::new(root), RecursiveMode::Recursive);
    }

    let app = app.clone();
    std::thread::spawn(move || {
        while rx.recv().is_ok() {
            // イベントの嵐を1回の通知にまとめる（400ms静かになるまで待つ）
            while rx.recv_timeout(Duration::from_millis(400)).is_ok() {}
            let _ = app.emit("tasks-changed", ());
        }
    });

    *guard = Some(watcher);
}

pub type WatcherHandle = Option<RecommendedWatcher>;
