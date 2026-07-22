// IPC wrapper around Tauri's `invoke` API.
//
// Every method mirrors a backend command and returns the raw response:
//   { success: boolean; data?: T; error?: string }

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { Folder, Rule, Settings, ActivityEntry, IpcResponse, OrganizeResult } from './types';

export const ipc = {
  files: {
    scan: (folderPath: string) =>
      invoke<IpcResponse<{ files: unknown[]; stats: { scanned: number; truncated: boolean } }>>(
        'scan',
        { folderPath },
      ),
    scanRecursive: (folderPath: string) => invoke<IpcResponse>('scan_recursive', { folderPath }),
    preview: (folderPath: string) => invoke<IpcResponse>('preview', { folderPath }),
    previewRecursive: (folderPath: string) =>
      invoke<IpcResponse>('preview_recursive', { folderPath }),
    organize: (folderPath: string) =>
      invoke<IpcResponse<OrganizeResult>>('organize', { folderPath }),
    organizeRecursive: (folderPath: string, opts?: Record<string, unknown>) =>
      invoke<IpcResponse<OrganizeResult>>('organize_recursive', { folderPath, opts }),
    cancelOrganize: (sessionId: string) => invoke<IpcResponse>('cancel', { sessionId }),
    undoOrganize: (sessionId: string) => invoke<IpcResponse>('undo', { sessionId }),
    dismissUndo: (sessionId: string) => invoke<IpcResponse>('dismiss_undo', { sessionId }),
    onProgress: (cb: (data: unknown) => void): Promise<UnlistenFn> =>
      listen('organize:progress', (e) => cb(e.payload)),
    onDone: (cb: (data: unknown) => void): Promise<UnlistenFn> =>
      listen('organize:done', (e) => cb(e.payload)),
  },

  folders: {
    list: () => invoke<IpcResponse<Folder[]>>('list_folders'),
    add: () => invoke<IpcResponse<Folder>>('add'),
    remove: (id: string) => invoke<IpcResponse>('remove', { id }),
    updateName: (id: string, name: string) =>
      invoke<IpcResponse<Folder>>('update_name', { id, name }),
    updateMode: (id: string, mode: string) =>
      invoke<IpcResponse<Folder>>('update_mode', { id, mode }),
    assignRules: (folderId: string, ruleIds: string[]) =>
      invoke<IpcResponse<Folder>>('assign_rules', { folderId, ruleIds }),
    unassignRules: (folderId: string, ruleIds: string[]) =>
      invoke<IpcResponse<Folder>>('unassign_rules', { folderId, ruleIds }),
    updateSubfoldersConfig: (id: string, config: unknown) =>
      invoke<IpcResponse<Folder>>('update_subfolders_config', { id, config }),
    updateSchedule: (id: string, schedule: unknown) =>
      invoke<IpcResponse<Folder>>('update_schedule', { id, schedule }),
    selectDestination: () => invoke<IpcResponse<string>>('select_destination'),
    confirmFirstRun: (id: string) => invoke<IpcResponse<Folder>>('confirm_first_run', { id }),
  },

  rules: {
    list: () => invoke<IpcResponse<Rule[]>>('list_rules'),
    getByIds: (ids: string[]) => invoke<IpcResponse<Rule[]>>('get_by_ids', { ids }),
    create: (rule: Rule) => invoke<IpcResponse<Rule>>('create', { rule }),
    update: (id: string, updates: Partial<Rule>) =>
      invoke<IpcResponse<Rule>>('update', { id, updates }),
    delete: (id: string) => invoke<IpcResponse>('delete_rule', { id }),
    toggle: (id: string) => invoke<IpcResponse<{ enabled: boolean }>>('toggle', { id }),
  },

  watcher: {
    start: (folderId: string) => invoke<IpcResponse>('start', { folderId }),
    stop: (folderId: string) => invoke<IpcResponse>('stop', { folderId }),
    pause: (folderId: string) => invoke<IpcResponse>('pause', { folderId }),
    resume: (folderId: string) => invoke<IpcResponse>('resume', { folderId }),
    status: (folderId: string) => invoke<IpcResponse<{ status: string }>>('status', { folderId }),
    onStatusChanged: (
      cb: (data: { folderId: string; status: string }) => void,
    ): Promise<UnlistenFn> => listen('watcher:status-changed', (e) => cb(e.payload as never)),
  },

  settings: {
    get: () => invoke<IpcResponse<Settings>>('get'),
    set: (settings: Record<string, unknown>) => invoke<IpcResponse>('set', { settings }),
  },

  activity: {
    list: (opts: {
      limit?: number;
      offset?: number;
      folderId?: string;
      ruleId?: string;
      success?: boolean;
      type?: string;
    }) => invoke<IpcResponse<{ items: ActivityEntry[]; total: number }>>('list_activity', { opts }),
    delete: (ids: string[]) => invoke<IpcResponse>('delete_activity', { ids }),
    clear: () => invoke<IpcResponse>('clear'),
  },

  permissions: {
    checkAccess: (folderPath: string) =>
      invoke<IpcResponse<{ hasAccess: boolean; message: string }>>('check_access', { folderPath }),
    openSettings: () => invoke<IpcResponse>('open_settings'),
    isMacOS: () => invoke<IpcResponse<boolean>>('is_macos'),
  },

  app: {
    getVersion: () => invoke<IpcResponse<string>>('get_version'),
    getIcon: () => invoke<IpcResponse<string>>('get_icon'),
    minimize: () => invoke('minimize'),
    maximize: () => invoke('maximize'),
    close: () => invoke('close'),
  },
};
