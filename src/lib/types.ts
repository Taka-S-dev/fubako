export type Status = 'backlog' | 'doing' | 'done';
export type ProgressMode = 'auto' | 'manual';

export interface ChecklistItem {
  text: string;
  done: boolean;
  estimate_min: number | null;
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
  last_activity: string | null;
  file_count: number;
  file_names: string[];
  stale: boolean;
}

export interface FolderEntry {
  name: string;
  is_dir: boolean;
  size: number;
  modified: string | null;
}

export interface AppConfig {
  work_root: string | null;
  archive_root: string | null;
  hotkey: string;
  stale_days: number;
  template_files: string[];
  /** 隠し設定: config.json 直接編集でのみ変更。UI表記とタイトルを置き換える */
  display_name: string | null;
  deep_archive_root: string | null;
  deep_archive_months: number;
}
