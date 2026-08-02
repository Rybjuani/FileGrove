import type { TFunction } from 'i18next';

const ERROR_KEYS: Record<string, string> = {
  ACTIVITY_FINALIZE_FAILED: 'errors.activityFinalizeFailed',
  ACTIVITY_JOURNAL_FAILED: 'errors.activityJournalFailed',
  AUTO_MODE_CONFLICTS_WITH_SUBFOLDERS: 'errors.autoModeConflictsWithSubfolders',
  COMMAND_CANCELLED: 'errors.commandCancelled',
  COMMAND_INVALID: 'errors.invalidRuleAction',
  COMMAND_INTERRUPTED_REVIEW_REQUIRED: 'errors.commandInterruptedReview',
  COMMAND_LEDGER_FAILED: 'errors.commandLedgerFailed',
  COMMAND_RETRY_NOT_PENDING: 'errors.commandRetryNotFound',
  COMMAND_TIMED_OUT: 'errors.commandTimedOut',
  COMMAND_TERMINATED: 'errors.commandUnavailable',
  COMMAND_TIMEOUT_INVALID: 'errors.invalidCommandTimeout',
  COMMAND_UNAVAILABLE: 'errors.commandUnavailable',
  DELETE_FAILED: 'errors.deleteFailed',
  DELETE_IN_PROGRESS: 'errors.deleteInProgress',
  DELETE_INTERRUPTED_REVIEW_REQUIRED: 'errors.deleteInterruptedReview',
  DUPLICATE_CHANGED: 'errors.duplicateChanged',
  DESTINATION_CREATE_FAILED: 'errors.destinationCreateFailed',
  DESTINATION_RESTORE_FAILED: 'errors.undoBackupRestoreFailed',
  FIRST_RUN_CONFIRMATION_REQUIRED: 'errors.recursiveConfirmationRequired',
  FOLDER_NOT_FOUND: 'errors.folderNotFound',
  FOLDER_READ_DENIED: 'errors.folderReadDenied',
  FOLDER_WRITE_DENIED: 'errors.folderWriteDenied',
  INVALID_COMMAND_RETRY: 'errors.invalidCommandRetry',
  INVALID_DESTINATION: 'errors.invalidDestination',
  INVALID_DESTINATION_PROTECTED: 'errors.invalidDestinationProtected',
  INVALID_DESTINATION_TOKEN: 'errors.invalidDestinationToken',
  INVALID_DESTINATION_TOKEN_FORMAT: 'errors.invalidDestinationTokenFormat',
  INVALID_DESTINATION_TRAVERSAL: 'errors.invalidDestinationTraversal',
  INVALID_DESTINATION_WATCHED_FOLDER: 'errors.invalidDestinationWatchedFolder',
  INVALID_EXCLUDE_FOLDER: 'errors.invalidExcludeFolder',
  INVALID_EXCLUDE_PATTERN: 'errors.invalidExcludePattern',
  INVALID_FOLDER_NAME: 'errors.invalidFolderName',
  INVALID_MAX_DEPTH: 'errors.invalidMaxDepth',
  INVALID_MODE: 'errors.invalidMode',
  INVALID_RULE: 'errors.invalidRule',
  INVALID_RULE_ACTION: 'errors.invalidRuleAction',
  INVALID_RULE_CONDITIONS: 'errors.invalidRuleConditions',
  INVALID_RULE_ICON: 'errors.invalidRuleIcon',
  INVALID_RULE_NAME: 'errors.invalidRuleName',
  INVALID_RULE_ORDER: 'errors.invalidRuleOrder',
  INVALID_RULE_REGEX: 'errors.invalidRuleRegex',
  INVALID_SESSION_ID: 'errors.invalidSessionId',
  INVALID_SETTINGS: 'errors.invalidSettings',
  MOVE_CANCELLED: 'errors.moveCancelled',
  MOVE_FAILED: 'errors.moveFailed',
  NO_RULES_ASSIGNED: 'errors.noRulesAssigned',
  NO_RULES_ENABLED: 'errors.noRulesEnabled',
  ORGANIZE_ALREADY_RUNNING: 'errors.organizeAlreadyRunning',
  ORGANIZE_NOT_FOUND: 'errors.organizeNotFound',
  PLAN_NO_LONGER_VALID: 'errors.planNoLongerValid',
  RECURSIVE_SCAN_DISABLED: 'errors.subfoldersNotEnabled',
  RULE_NOT_FOUND: 'errors.ruleNotFound',
  RULE_SET_CHANGED: 'errors.ruleSetChanged',
  SCAN_FAILED: 'errors.scanFailed',
  SCAN_INCOMPLETE: 'errors.scanIncomplete',
  SCAN_LIMIT_REACHED: 'errors.scanLimitReached',
  SOURCE_FILE_CHANGED: 'errors.sourceFileChanged',
  SOURCE_FILE_INVALID: 'errors.fileUnavailable',
  SOURCE_FILE_OUTSIDE_WATCHED_FOLDER: 'errors.fileUnavailable',
  SUBFOLDERS_REQUIRE_MANUAL_MODE: 'errors.subfoldersRequireManualMode',
  TOO_MANY_EXCLUSIONS: 'errors.tooManyExclusions',
  TOO_MANY_RULES: 'errors.tooManyRules',
  TRASH_FAILED: 'errors.trashFailed',
  UNDO_ALREADY_RUNNING: 'errors.undoAlreadyRunning',
  UNDO_BACKUP_RESTORE_FAILED: 'errors.undoBackupRestoreFailed',
  UNDO_DESTINATION_ALREADY_EXISTS: 'errors.undoDestinationExists',
  UNDO_FAILED: 'errors.undoFailed',
  UNDO_JOURNAL_NEEDS_REVIEW: 'errors.undoJournalNeedsReview',
  UNDO_LOG_NOT_FOUND: 'errors.undoLogNotFound',
  UNDO_NEWER_SESSION_EXISTS: 'errors.undoNewerSessionExists',
  UNDO_ORIGIN_EXISTS: 'errors.undoOriginExists',
  UNDO_ORIGIN_ALREADY_EXISTS: 'errors.undoOriginExists',
  UNDO_OVERWRITE_BACKUP_MISSING: 'errors.undoBackupMissing',
  UNDO_PARTIAL_FAILURE: 'errors.undoPartialFailure',
  USER_CANCELED_FILE_SELECT: 'errors.userCanceledFileSelect',
  USER_CANCELED_FOLDER_SELECT: 'errors.userCanceledFolderSelect',
  WATCHED_FOLDER_INVALID: 'errors.watchedFolderInvalid',
  WATCHED_FOLDER_OVERLAP: 'errors.watchedFolderOverlap',
  WATCHED_FOLDER_PROTECTED: 'errors.watchedFolderProtected',
  WATCHER_NOT_RUNNING: 'errors.watcherNotRunning',
  WATCHER_REQUIRES_AUTO_MODE: 'errors.watcherRequiresAutoMode',
};

export function readableIpcError(error: unknown, t: TFunction): string {
  if (typeof error !== 'string' || !error.trim()) return t('errors.generic');
  const [rawCode, ...detailParts] = error.split(':');
  const code = rawCode.trim().toUpperCase();
  const detail = detailParts.join(':').trim();
  if (code === 'COMMAND_EXIT') {
    return t('errors.commandExit', { code: detail || '?' });
  }
  const key = ERROR_KEYS[code];
  if (key) return t(key);
  if (code.startsWith('MOVE_') || code.startsWith('OVERWRITE_')) return t('errors.moveFailed');
  if (code.startsWith('UNDO_')) return t('errors.undoFailed');
  if (code.startsWith('SCAN_')) return t('errors.scanFailed');
  if (code.startsWith('DELETE_')) return t('errors.deleteFailed');
  if (code.startsWith('TRASH_')) return t('errors.trashFailed');
  if (code.startsWith('DESTINATION_')) return t('errors.invalidDestination');
  return detail ? `${t('errors.generic')}: ${detail}` : t('errors.generic');
}

export function ipcErrorCode(error: unknown): string | null {
  if (typeof error !== 'string') return null;
  const code = error.split(':', 1)[0]?.trim().toUpperCase();
  return code || null;
}
