import { invoke } from '@tauri-apps/api/core';
import type { AppConfig, FolderListing, MetaPatch, Task } from './types';

export const api = {
  getConfig: () => invoke<AppConfig>('get_config'),
  /** 保存は成功しても伝えることがあれば文言が返る（ホットキーを取られていた等） */
  setConfig: (config: AppConfig) => invoke<string | null>('set_config', { config }),
  takeStartupWarning: () => invoke<string | null>('take_startup_warning'),
  listTasks: () => invoke<Task[]>('list_tasks'),
  createTask: (name: string, useTemplate: boolean) =>
    invoke<Task>('create_task', { name, useTemplate }),
  setTaskMeta: (path: string, patch: MetaPatch) =>
    invoke<void>('set_task_meta', {
      path,
      patch: {
        status: patch.status,
        tags: patch.tags,
        due: patch.due,
        memo: patch.memo,
        checklist: patch.checklist,
        manual_progress: patch.manualProgress,
        progress_mode: patch.progressMode,
        on_hold: patch.onHold,
      },
    }),
  renameTask: (path: string, name: string) => invoke<string>('rename_task', { path, name }),
  completeTask: (path: string) => invoke<string>('complete_task', { path }),
  importTask: (path: string) => invoke<Task>('import_task', { path }),
  deepArchiveTask: (path: string) => invoke<string>('deep_archive_task', { path }),
  reopenTask: (path: string) => invoke<string>('reopen_task', { path }),
  openInExplorer: (path: string) => invoke<void>('open_in_explorer', { path }),
  openEntry: (path: string) => invoke<void>('open_entry', { path }),
  listFolder: (path: string) => invoke<FolderListing>('list_folder', { path }),
  getAutostart: () => invoke<boolean>('get_autostart'),
  setAutostart: (enabled: boolean) => invoke<void>('set_autostart', { enabled }),
};
