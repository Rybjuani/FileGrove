// Shared IPC types — mirror of the Rust domain types in src-tauri/src/domain/.

export type FolderMode = 'single' | 'auto';
export type WatcherStatus = 'stopped' | 'running' | 'paused';
export type DestinationMode = 'flatten' | 'preserve';
export type ActivityType = 'move' | 'trash' | 'delete' | 'runCommand';
export type MaxDepth = 'unlimited' | number | { level: number };

export interface SubfolderConfig {
  enabled: boolean;
  destinationMode: DestinationMode;
  firstRunConfirmed: boolean;
  maxDepth: MaxDepth;
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

export interface ActivityFilters {
  limit?: number;
  offset?: number;
  folderId?: string;
  ruleId?: string;
  success?: boolean;
  type?: ActivityType;
  since?: string;
  until?: string;
}

export interface CommandTestResult {
  filePath: string;
  exitCode: number | null;
  output: string;
  durationMs: number;
  timedOut: boolean;
}

export interface FileItem {
  path: string;
  name: string;
  extension: string;
  size: number;
  modifiedAt: string;
  createdAt: string;
}

export interface PlannedAction {
  file: FileItem;
  ruleId: string | null;
  ruleName: string | null;
  action: Action | null;
}

export interface ScanStats {
  scanned: number;
  errors: number;
  truncated: boolean;
  cancelled: boolean;
}

export interface PreviewResult {
  plan: PlannedAction[];
  stats: ScanStats;
}

export interface UndoSessionSummary {
  sessionId: string;
  at: string;
  entryCount: number;
  fromPath: string;
}

export type Settings = Record<string, unknown> & {
  theme?: 'dark' | 'light' | 'system';
  language?: string;
  launchAtStartup?: boolean;
  minimizeToTray?: boolean;
  showTrayIcon?: boolean;
  notifications?: boolean;
  automationPaused?: boolean;
  conflictBehavior?: 'overwrite' | 'rename' | 'skip';
  duplicateBehavior?: 'keepBoth' | 'trash' | 'delete';
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

// ─── Deduplication ───────────────────────────────────────────────────────────

export interface DuplicateFile {
  path: string;
  name: string;
  size: number;
  modifiedAt: string;
  isOriginal: boolean;
}

export interface DuplicateGroup {
  hash: string;
  size: number;
  original: DuplicateFile;
  duplicates: DuplicateFile[];
}

export interface DuplicateReport {
  groups: DuplicateGroup[];
  totalDuplicates: number;
  totalWastedBytes: number;
}

export interface TrashResult {
  trashed: number;
  failed: number;
  errors: string[];
}
