use crate::model::AppConfig;
use crate::AppState;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

/// 変化が途切れたとみなすまでの静止時間
const QUIET: Duration = Duration::from_millis(400);
/// 静止を待ち続けて更新が止まらないための上限。
/// 大きなフォルダを作業ディレクトリへコピーしている間はイベントが途切れないため、
/// 静止だけを条件にするとコピーが終わるまで一覧が更新されない
const MAX_WAIT: Duration = Duration::from_secs(2);

/// イベントの嵐が落ち着くまで待つ。ただし最大待ち時間で打ち切る。
/// 送信側が閉じた場合もすぐ戻る
fn settle(rx: &mpsc::Receiver<()>, quiet: Duration, max: Duration) {
    let start = Instant::now();
    while start.elapsed() < max {
        if rx.recv_timeout(quiet).is_err() {
            return;
        }
    }
}

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
            settle(&rx, QUIET, MAX_WAIT);
            let _ = app.emit("tasks-changed", ());
        }
    });

    *guard = Some(watcher);
}

pub type WatcherHandle = Option<RecommendedWatcher>;

#[cfg(test)]
mod tests {
    use super::*;

    const QUIET_T: Duration = Duration::from_millis(50);
    const MAX_T: Duration = Duration::from_millis(300);

    #[test]
    fn settle_returns_once_the_events_stop() {
        let (tx, rx) = mpsc::channel::<()>();
        tx.send(()).unwrap();
        let start = Instant::now();
        settle(&rx, QUIET_T, MAX_T);
        // 静止したのだから、上限まで待たずに戻る
        assert!(start.elapsed() < MAX_T, "戻るのが遅すぎる: {:?}", start.elapsed());
        drop(tx);
    }

    #[test]
    fn settle_gives_up_waiting_during_a_long_copy() {
        let (tx, rx) = mpsc::channel::<()>();
        // 静止時間より短い間隔でイベントを出し続ける（コピー中の状態）
        let sender = std::thread::spawn(move || {
            for _ in 0..40 {
                if tx.send(()).is_err() {
                    return;
                }
                std::thread::sleep(QUIET_T / 5);
            }
        });
        let start = Instant::now();
        settle(&rx, QUIET_T, MAX_T);
        let waited = start.elapsed();
        // 静止しなくても上限で打ち切られる。待ち続けると一覧が更新されない
        assert!(waited >= MAX_T, "早すぎる: {waited:?}");
        assert!(waited < MAX_T * 3, "打ち切られていない: {waited:?}");
        sender.join().unwrap();
    }

    #[test]
    fn settle_returns_when_the_watcher_is_replaced() {
        let (tx, rx) = mpsc::channel::<()>();
        drop(tx);
        let start = Instant::now();
        settle(&rx, QUIET_T, MAX_T);
        // 送信側が閉じたら待つ意味がない（設定変更で watcher を作り直したとき）
        assert!(start.elapsed() < MAX_T, "閉じても待っている: {:?}", start.elapsed());
    }
}
