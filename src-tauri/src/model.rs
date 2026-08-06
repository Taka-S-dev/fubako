use serde::{Deserialize, Serialize};

/// タスクの進行状態。Done はアーカイブ済み（フォルダの物理位置で決まる）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    #[default]
    Backlog,
    Doing,
    Done,
}

/// 進捗率の出どころ。auto=チェックリストから計算 / manual=手動入力
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProgressMode {
    #[default]
    Auto,
    Manual,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistItem {
    pub text: String,
    #[serde(default)]
    pub done: bool,
    /// 見積(分)。未入力の項目は他項目の平均（全て未入力なら均等）で重み付けされる
    #[serde(default)]
    pub estimate_min: Option<u32>,
}

/// フォルダに入れられない置き場所への参照。共有ドライブや
/// チケットの URL など、作業に紐づくが中に持てないもの
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskLink {
    pub url: String,
    /// 表示名。空なら url をそのまま出す
    #[serde(default)]
    pub label: String,
}

/// 各作業フォルダ内の .todo.json に保存するメタデータ。
/// フォルダを手動で移動してもデータが追従するよう、フォルダ内に持つ。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TaskMeta {
    #[serde(default)]
    pub status: Status,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub due: Option<String>,
    #[serde(default)]
    pub memo: String,
    #[serde(default)]
    pub checklist: Vec<ChecklistItem>,
    /// 手動入力の進捗率
    #[serde(default)]
    pub progress: Option<u32>,
    #[serde(default)]
    pub progress_mode: ProgressMode,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub completed_at: Option<String>,
    /// 保留を始めた日時。None なら保留していない。
    /// 真偽値ではなく時刻にしておくと、保留がいつからかを表示でき、
    /// completed_at と同じ「状態は時刻で持つ」形に揃う
    #[serde(default)]
    pub on_hold_since: Option<String>,
    #[serde(default)]
    pub links: Vec<TaskLink>,
}

/// フロントエンドへ返すタスク一件分
#[derive(Debug, Clone, Serialize)]
pub struct Task {
    pub path: String,
    pub folder_name: String,
    pub name: String,
    pub date_prefix: Option<String>,
    pub status: Status,
    pub archived: bool,
    pub tags: Vec<String>,
    pub due: Option<String>,
    pub memo: String,
    pub checklist: Vec<ChecklistItem>,
    pub manual_progress: Option<u32>,
    pub progress_mode: ProgressMode,
    /// 表示用: チェックリストがあれば達成率、なければ手動値
    pub progress: Option<u32>,
    pub checklist_done: usize,
    pub checklist_total: usize,
    /// 未完了項目の見積合計(分)。見積が1つも無ければ None
    pub remaining_min: Option<u32>,
    pub created_at: Option<String>,
    pub completed_at: Option<String>,
    pub on_hold_since: Option<String>,
    pub links: Vec<TaskLink>,
    pub last_activity: Option<String>,
    /// フォルダ直下の項目数
    pub file_count: usize,
    /// 検索用のファイル・フォルダ名。サブフォルダの中も `資料\仕様書.xlsx` の形で含む
    pub file_names: Vec<String>,
    pub stale: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct FolderEntry {
    /// タスクフォルダからの相対パス。サブフォルダの中身は `資料\仕様書.xlsx` のように出る
    pub rel: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: Option<String>,
    /// シェルが持つ種類アイコンの data URI。取得できなければ None
    pub icon: Option<String>,
}

/// フォルダの中身の一覧。上限で打ち切った場合はその旨を伝え、
/// 「これで全部」と誤解させないようにする
#[derive(Debug, Clone, Serialize)]
pub struct FolderListing {
    pub entries: Vec<FolderEntry>,
    /// 深さの上限を超えるフォルダがあった
    pub deeper_omitted: bool,
    /// 件数の上限に達して打ち切った
    pub count_capped: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub work_root: Option<String>,
    pub archive_root: Option<String>,
    #[serde(default = "default_hotkey")]
    pub hotkey: String,
    #[serde(default = "default_stale_days")]
    pub stale_days: u32,
    #[serde(default = "default_templates")]
    pub template_files: Vec<String>,
    /// 隠し設定: ウィンドウタイトルとヘッダー表記を置き換える。
    /// 設定UIには出さず config.json の直接編集でのみ変更する。空文字で表記を消す
    #[serde(default)]
    pub display_name: Option<String>,
    /// 古い完了タスクの退避先。ここはスキャン対象外（コールドストレージ）
    #[serde(default)]
    pub deep_archive_root: Option<String>,
    #[serde(default = "default_deep_archive_months")]
    pub deep_archive_months: u32,
}

fn default_deep_archive_months() -> u32 {
    6
}

fn default_hotkey() -> String {
    "Ctrl+Alt+KeyE".into()
}

fn default_stale_days() -> u32 {
    14
}

fn default_templates() -> Vec<String> {
    vec!["メモ.md".into()]
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            work_root: None,
            archive_root: None,
            hotkey: default_hotkey(),
            stale_days: default_stale_days(),
            template_files: default_templates(),
            display_name: None,
            deep_archive_root: None,
            deep_archive_months: default_deep_archive_months(),
        }
    }
}
