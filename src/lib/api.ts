import { invoke } from '@tauri-apps/api/core';
import type { AppConfig, ChecklistItem, FolderEntry, ProgressMode, Status, Task } from './types';

export const api = {
  getConfig: () => invoke<AppConfig>('get_config'),
  setConfig: (config: AppConfig) => invoke<void>('set_config', { config }),
  listTasks: () => invoke<Task[]>('list_tasks'),
  createTask: (name: string, useTemplate: boolean) =>
    invoke<Task>('create_task', { name, useTemplate }),
  setTaskMeta: (
    path: string,
    status: Status,
    tags: string[],
    due: string | null,
    memo: string,
    checklist: ChecklistItem[],
    manualProgress: number | null,
    progressMode: ProgressMode
  ) =>
    invoke<void>('set_task_meta', {
      path,
      patch: {
        status,
        tags,
        due,
        memo,
        checklist,
        manual_progress: manualProgress,
        progress_mode: progressMode,
      },
    }),
  renameTask: (path: string, name: string) => invoke<string>('rename_task', { path, name }),
  completeTask: (path: string) => invoke<string>('complete_task', { path }),
  importTask: (path: string) => invoke<Task>('import_task', { path }),
  deepArchiveTask: (path: string) => invoke<string>('deep_archive_task', { path }),
  reopenTask: (path: string) => invoke<string>('reopen_task', { path }),
  openInExplorer: (path: string) => invoke<void>('open_in_explorer', { path }),
  openEntry: (path: string) => invoke<void>('open_entry', { path }),
  listFolder: (path: string) => invoke<FolderEntry[]>('list_folder', { path }),
  getAutostart: () => invoke<boolean>('get_autostart'),
  setAutostart: (enabled: boolean) => invoke<void>('set_autostart', { enabled }),
};
