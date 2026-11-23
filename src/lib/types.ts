export type Status = 'backlog' | 'doing' | 'done';

export type ListSortKey = 'status' | 'name' | 'date' | 'progress' | 'due' | 'activity';

/** ビュー切り替えで失われないよう、各ビューの表示条件は親が保持する */
export interface ListViewState {
  statusFilter: 'all' | Status;
  sortKey: ListSortKey;
  sortAsc: boolean;
}

export interface CalendarViewState {
  year: number;
  month: number;
  showStart: boolean;
  showDue: boolean;
  showDone: boolean;
}
export type ProgressMode = 'auto' | 'manual';

export interface ChecklistItem {
  text: string;
  done: boolean;
  estimate_min: number | null;
}

/** フォルダに入れられない置き場所への参照（共有フォルダ・チケットの URL など） */
export interface TaskLink {
  url: string;
  /** 表示名。空なら url をそのまま出す */
  label: string;
}

export interface Task {
  /** 作業の id。名前を変えても移しても変わらない。振られる前の一瞬だけ null */
  id: string | null;
  path: string;
  folder_name: string;
  name: string;
  date_prefix: string | null;
  status: Status;
  archived: boolean;
  tags: string[];
  due: string | null;
  memo: string;
  checklist: ChecklistItem[];
  manual_progress: number | null;
  progress_mode: ProgressMode;
  progress: number | null;
  checklist_done: number;
  checklist_total: number;
  remaining_min: number | null;
  created_at: string | null;
  completed_at: string | null;
  /** 保留を始めた日時。null なら保留していない */
  on_hold_since: string | null;
  links: TaskLink[];
  /** 読んだときの .todo.json の指紋。保存のときに baseRev として返す */
  meta_rev: string;
  last_activity: string | null;
  file_count: number;
  file_names: string[];
  stale: boolean;
}

/**
 * 作業の記録に保存する項目。書いた項目だけが書き換わり、書かなかった項目は
 * ファイルにある値のまま残る。期限と手動の進捗は null で「消す」になる
 */
export interface MetaPatch {
  status?: Status;
  tags?: string[];
  due?: string | null;
  memo?: string;
  checklist?: ChecklistItem[];
  manualProgress?: number | null;
  progressMode?: ProgressMode;
  onHold?: boolean;
  links?: TaskLink[];
  /**
   * 読んだときの指紋（Task.meta_rev）。渡すと、その後ほかで書き換えられていたとき
   * 保存せずに 'CONFLICT' で断られる。手元の値を書き戻す保存では必ず渡す
   */
  baseRev?: string;
}

/** 画面の地の明暗。auto は OS の設定に従う */
export type Theme = 'auto' | 'light' | 'dark';

export type ViewName = 'board' | 'list' | 'cal' | 'dash';

export interface FolderEntry {
  /** タスクフォルダからの相対パス。サブフォルダの中身は `資料\仕様書.xlsx` のように入る */
  rel: string;
  is_dir: boolean;
  size: number;
  modified: string | null;
  /** シェルが持つ種類アイコンの data URI。取得できなければ null */
  icon: string | null;
}

export interface FolderListing {
  entries: FolderEntry[];
  /** 深さの上限を超えるフォルダがあった */
  deeper_omitted: boolean;
  /** 件数の上限に達して打ち切った */
  count_capped: boolean;
}

/** ディープアーカイブを読んだ結果。上限で打ち切ったかどうかも持つ */
export interface DeepArchiveListing {
  tasks: Task[];
  capped: boolean;
}

/** 保存した絞り込み。クエリは検索欄に打つものと同じ文法 */
export interface SavedFilter {
  name: string;
  query: string;
}

export interface AppConfig {
  work_root: string | null;
  archive_root: string | null;
  hotkey: string;
  stale_days: number;
  template_files: string[];
  /** タスクに属さないが日常的に開く場所 */
  places: string[];
  /** 名前を付けて保存した絞り込み。漏斗のメニューから選ぶと、検索欄とは別に一覧に効く */
  saved_filters: SavedFilter[];
  theme: Theme;
  /** 前回のウィンドウの大きさと位置。アプリが書き、設定画面は触らない */
  window: {
    x: number;
    y: number;
    width: number;
    height: number;
    maximized: boolean;
  } | null;
  /** 隠し設定: config.json 直接編集でのみ変更。UI表記とタイトルを置き換える */
  display_name: string | null;
  deep_archive_root: string | null;
  deep_archive_months: number;
  /**
   * 期限つきの作業を Tasks プラグインの書き方で書き出す md のパス。null なら書き出さない。
   * 一方通行で、md を直しても Fubako には戻らない
   */
  export_md: string | null;
  /** 保留中の作業も書き出すか */
  export_include_on_hold: boolean;
}
