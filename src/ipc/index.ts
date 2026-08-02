// IPC wrapper around Tauri's `invoke` API.
//
// Every method mirrors a backend command and returns the raw response:
//   { success: boolean; data?: T; error?: string }

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  Folder,
  Rule,
  Settings,
  ActivityEntry,
  IpcResponse,
  OrganizeResult,
  OrganizeStarted,
  OrganizeProgress,
  OrganizeDone,
  ActivityFilters,
  SubfolderConfig,
  CommandTestResult,
  PreviewResult,
  UndoSessionSummary,
} from './types';

export const ipc = {
  files: {
    scan: (folderPath: string) =>
      invoke<IpcResponse<{ files: unknown[]; stats: { scanned: number; truncated: boolean } }>>(
        'scan',
        { folderPath },
      ),
    scanRecursive: (folderPath: string) => invoke<IpcResponse>('scan_recursive', { folderPath }),
    preview: (folderPath: string) => invoke<IpcResponse<PreviewResult>>('preview', { folderPath }),
    previewRecursive: (folderPath: string) =>
      invoke<IpcResponse<PreviewResult>>('preview_recursive', { folderPath }),
    organize: (folderPath: string) =>
      invoke<IpcResponse<OrganizeResult>>('organize', { folderPath }),
    organizeRecursive: (folderPath: string, opts?: Record<string, unknown>) =>
      invoke<IpcResponse<OrganizeResult>>('organize_recursive', { folderPath, opts }),
    cancelOrganize: (sessionId: string) => invoke<IpcResponse>('cancel', { sessionId }),
    latestUndoSession: (folderId: string) =>
      invoke<IpcResponse<UndoSessionSummary | null>>('latest_undo_session', { folderId }),
    listUndoSessions: (folderId: string) =>
      invoke<IpcResponse<UndoSessionSummary[]>>('list_undo_sessions', { folderId }),
    undoOrganize: (sessionId: string) =>
      invoke<IpcResponse<{ done: number; failed: number; total: number }>>('undo', { sessionId }),
    dismissUndo: (sessionId: string) => invoke<IpcResponse>('dismiss_undo', { sessionId }),
    onStarted: (cb: (data: OrganizeStarted) => void): Promise<UnlistenFn> =>
      listen('organize:started', (e) => cb(e.payload as OrganizeStarted)),
    onProgress: (cb: (data: OrganizeProgress) => void): Promise<UnlistenFn> =>
      listen('organize:progress', (e) => cb(e.payload as OrganizeProgress)),
    onDone: (cb: (data: OrganizeDone) => void): Promise<UnlistenFn> =>
      listen('organize:done', (e) => cb(e.payload as OrganizeDone)),
    onBackgroundError: (
      cb: (data: { folderPath: string; trigger: string; error: string }) => void,
    ): Promise<UnlistenFn> =>
      listen('background:error', (e) =>
        cb(e.payload as { folderPath: string; trigger: string; error: string }),
      ),
  },

  folders: {
    list: () => invoke<IpcResponse<Folder[]>>('list_folders'),
    add: () => invoke<IpcResponse<Folder>>('add'),
    remove: (id: string) => invoke<IpcResponse>('remove', { id }),
    updateName: (id: string, name: string) =>
      invoke<IpcResponse<Folder>>('update_name', { id, name }),
    updateMode: (id: string, mode: string, disableSubfolders = false) =>
      invoke<IpcResponse<Folder>>('update_mode', { id, mode, disableSubfolders }),
    assignRules: (folderId: string, ruleIds: string[]) =>
      invoke<IpcResponse<Folder>>('assign_rules', { folderId, ruleIds }),
    unassignRules: (folderId: string, ruleIds: string[]) =>
      invoke<IpcResponse<Folder>>('unassign_rules', { folderId, ruleIds }),
    applyRecommendedRules: (folderId: string) =>
      invoke<IpcResponse<Folder>>('apply_recommended_rules', { folderId }),
    updateSubfoldersConfig: (id: string, config: SubfolderConfig) =>
      invoke<IpcResponse<Folder>>('update_subfolders_config', { id, config }),
    updateExclusions: (id: string, excludeFolders: string[], excludePatterns: string[]) =>
      invoke<IpcResponse<Folder>>('update_exclusions', {
        id,
        excludeFolders,
        excludePatterns,
      }),
    updateConfiguration: (
      id: string,
      config: SubfolderConfig,
      excludeFolders: string[],
      excludePatterns: string[],
      ruleIds: string[],
      confirmRecursive = false,
    ) =>
      invoke<IpcResponse<Folder>>('update_configuration', {
        id,
        config,
        excludeFolders,
        excludePatterns,
        ruleIds,
        confirmRecursive,
      }),
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
    reorder: (ids: string[]) => invoke<IpcResponse>('reorder_rules', { ids }),
    testCommand: (command: string, timeoutMs: number) =>
      invoke<IpcResponse<CommandTestResult>>('test_command', { command, timeoutMs }),
    allowInterruptedRetry: (ruleId: string, filePath: string, command: string) =>
      invoke<IpcResponse>('allow_interrupted_retry', { ruleId, filePath, command }),
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
    list: (opts: ActivityFilters) =>
      invoke<IpcResponse<{ items: ActivityEntry[]; total: number }>>('list_activity', { opts }),
    delete: (ids: string[]) => invoke<IpcResponse>('delete_activity', { ids }),
    clear: () => invoke<IpcResponse>('clear'),
    onChanged: (cb: () => void): Promise<UnlistenFn> => listen('activity:changed', cb),
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
