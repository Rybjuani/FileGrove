// Shared IPC types — mirror of the Rust domain types in src-tauri/src/domain/.

export type FolderMode = 'single' | 'auto' | 'scheduled';
export type WatcherStatus = 'stopped' | 'running' | 'paused';
export type DestinationMode = 'flatten' | 'preserve';
export type ActivityType = 'move' | 'trash' | 'delete' | 'runCommand';

export interface SubfolderConfig {
  enabled: boolean;
  destinationMode: DestinationMode;
  firstRunConfirmed: boolean;
  maxDepth: 'unlimited' | number;
}

export interface Schedule {
  enabled: boolean;
  daysOfWeek: number[]; // 0 = Sunday, 6 = Saturday
  time: string; // "HH:MM" 24h
}

export interface Folder {
  id: string;
  path: string;
  name: string;
  mode: FolderMode;
  ruleIds: string[];
  watcherStatus: WatcherStatus;
  createdAt: string; // ISO 8601
  subfolders: SubfolderConfig;
  schedule: Schedule | null;
  excludeFolders: string[];
  excludePatterns: string[];
}

export type Condition =
  | { type: 'extension'; value: string[] }
  | { type: 'nameContains'; value: string[] }
  | { type: 'nameStartsWith'; value: string[] }
  | { type: 'nameEndsWith'; value: string[] }
  | { type: 'regex'; value: string[] }
  | { type: 'sizeGreaterThan'; value: string[] }
  | { type: 'sizeLessThan'; value: string[] }
  | { type: 'olderThan'; value: string[] }
  | { type: 'newerThan'; value: string[] };

export type Action =
  | { type: 'move'; destination: string }
  | { type: 'trash' }
  | { type: 'delete' }
  | { type: 'runCommand'; command: string; timeoutMs?: number; rerunAlways?: boolean };

export interface Rule {
  id: string;
  name: string;
  isQuickRule: boolean;
  enabled: boolean;
  order: number;
  icon: string;
  conditions: Condition[];
  action: Action;
  createdAt: string;
}

export interface ActivityEntry {
  id: string;
  at: string;
  type: ActivityType;
  fileName: string;
  filePath: string;
  folderId?: string;
  ruleId?: string;
  ruleName?: string;
  destination?: string;
  command?: string;
  exitCode?: number;
  output?: string;
  durationMs?: number;
  timedOut?: boolean;
  success: boolean;
  error?: string;
}

export type Settings = Record<string, unknown> & {
  theme?: 'dark' | 'light' | 'system';
  language?: string;
  launchAtStartup?: boolean;
  minimizeToTray?: boolean;
  showTrayIcon?: boolean;
  notifications?: boolean;
  conflictBehavior?: 'overwrite' | 'rename' | 'skip';
  onboardingCompleted?: boolean;
  rulesSortOrder?: 'created' | 'name';
  foldersSortOrder?: 'created' | 'name';
};

export interface IpcResponse<T = unknown> {
  success: boolean;
  data?: T;
  error?: string;
  requiresUpgrade?: boolean;
}

export interface OrganizeResult {
  sessionId: string;
  cancelled: boolean;
  undoCount: number;
  stats: { done: number; failed: number; total: number };
}

export interface OrganizeStarted {
  sessionId: string;
  folderPath: string;
}

export interface OrganizeProgress extends OrganizeStarted {
  done: number;
  failed: number;
  processed: number;
  total: number;
}

export interface OrganizeDone extends OrganizeStarted {
  cancelled: boolean;
  stats: { done: number; failed: number; total: number };
  error?: string;
}
