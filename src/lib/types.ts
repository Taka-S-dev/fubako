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
  last_activity: string | null;
  file_count: number;
  file_names: string[];
  stale: boolean;
}

/**
 * 詳細パネルから保存できる項目。まとめて1つの型にしてあるので、
 * 項目が増えても呼び出し側の引数の並びが崩れない
 */
export interface MetaPatch {
  status: Status;
  tags: string[];
  due: string | null;
  memo: string;
  checklist: ChecklistItem[];
  manualProgress: number | null;
  progressMode: ProgressMode;
  onHold: boolean;
  links: TaskLink[];
}

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

export interface AppConfig {
  work_root: string | null;
  archive_root: string | null;
  hotkey: string;
  stale_days: number;
  template_files: string[];
  /** タスクに属さないが日常的に開く場所 */
  places: string[];
  /** 隠し設定: config.json 直接編集でのみ変更。UI表記とタイトルを置き換える */
  display_name: string | null;
  deep_archive_root: string | null;
  deep_archive_months: number;
}
