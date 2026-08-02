import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  AlertTriangle,
  Eye,
  Files,
  Folder as FolderIcon,
  Pause,
  Pencil,
  Play,
  RotateCcw,
  Square,
  Trash2,
  Sparkles,
  Terminal,
  X,
} from 'lucide-react';
import { useAppStore } from '../stores/useAppStore';
import { ipc } from '../ipc';
import type {
  Action,
  Folder,
  OrganizeProgress,
  PlannedAction,
  PreviewResult,
  UndoSessionSummary,
} from '../ipc/types';
import { FolderAdvancedSettings } from './FolderAdvancedSettings';
import { readableIpcError } from '../lib/errors';

interface Props {
  folder: Folder;
  onNavigateToRules: () => void;
}

export function FolderCard({ folder, onNavigateToRules }: Props) {
  const { t, i18n } = useTranslation();
  const { rules, removeFolder, updateFolderName, updateFolderMode, applyRecommendedRules } =
    useAppStore();
  const [busy, setBusy] = useState(false);
  const [activeSessionId, setActiveSessionId] = useState<string | null>(null);
  const [cancelRequested, setCancelRequested] = useState(false);
  const [progress, setProgress] = useState<OrganizeProgress | null>(null);
  const [undoSession, setUndoSession] = useState<UndoSessionSummary | null>(null);
  const [undoSessionCount, setUndoSessionCount] = useState(0);
  const [undoBusy, setUndoBusy] = useState(false);
  const [lastResult, setLastResult] = useState<string | null>(null);
  const [previewOpen, setPreviewOpen] = useState(false);
  const [previewLoading, setPreviewLoading] = useState(false);
  const [previewResult, setPreviewResult] = useState<PreviewResult | null>(null);
  const [previewError, setPreviewError] = useState<string | null>(null);
  const [modeChanging, setModeChanging] = useState(false);
  const [installingRecommendations, setInstallingRecommendations] = useState(false);
  const busyRef = useRef(false);
  const activeSessionRef = useRef<string | null>(null);

  useEffect(() => {
    const off = ipc.watcher.onStatusChanged((data) => {
      if (data.folderId === folder.id) {
        void useAppStore.getState().loadFolders();
      }
    });
    return () => void off.then((fn) => fn());
  }, [folder.id]);

  useEffect(() => {
    let disposed = false;
    void ipc.files.listUndoSessions(folder.id).then((response) => {
      if (!disposed && response.success) {
        const sessions = response.data ?? [];
        setUndoSession(sessions[0] ?? null);
        setUndoSessionCount(sessions.length);
      }
    });
    return () => {
      disposed = true;
    };
  }, [folder.id]);

  useEffect(() => {
    const offStarted = ipc.files.onStarted((data) => {
      if (data.folderPath !== folder.path || !busyRef.current || activeSessionRef.current) return;
      activeSessionRef.current = data.sessionId;
      setActiveSessionId(data.sessionId);
    });
    const offProgress = ipc.files.onProgress((data) => {
      if (data.sessionId === activeSessionRef.current) {
        setProgress(data);
      }
    });

    return () => {
      void offStarted.then((fn) => fn());
      void offProgress.then((fn) => fn());
    };
  }, [folder.path]);

  const requestPreview = () =>
    folder.subfolders.enabled
      ? ipc.files.previewRecursive(folder.path)
      : ipc.files.preview(folder.path);

  const hasAssignedDangerousRule = rules.some(
    (rule) =>
      rule.enabled &&
      folder.ruleIds.includes(rule.id) &&
      (rule.action.type === 'delete' || rule.action.type === 'runCommand'),
  );

  const confirmDangerousImpact = async (context: 'manual' | 'auto'): Promise<boolean> => {
    if (!hasAssignedDangerousRule) return true;

    const response = await requestPreview();
    if (!response.success || !response.data) {
      const message = readableIpcError(response.error, t);
      setPreviewError(message);
      setPreviewResult(null);
      setPreviewOpen(true);
      setLastResult(message);
      return false;
    }
    setPreviewResult(response.data);
    setPreviewError(null);
    const dangerous = response.data.plan.filter(
      (item) => item.action?.type === 'delete' || item.action?.type === 'runCommand',
    );
    if (response.data.stats.truncated || response.data.stats.errors > 0) {
      setPreviewOpen(true);
      setLastResult(t('folderCard.preview.dangerousPreviewBlocked'));
      return false;
    }
    if (dangerous.length === 0) return true;

    const key =
      context === 'manual'
        ? 'folderCard.preview.dangerousManualConfirm'
        : 'folderCard.preview.dangerousAutoConfirm';
    const examples = dangerous
      .slice(0, 10)
      .map((item) => `• ${item.file.name} — ${t(`rules.action.${item.action!.type}`)}`)
      .join('\n');
    const remaining =
      dangerous.length > 10
        ? `\n${t('folderCard.preview.remaining', { count: dangerous.length - 10 })}`
        : '';
    return window.confirm(`${t(key, { count: dangerous.length })}\n\n${examples}${remaining}`);
  };

  const ensureRecursiveConfirmation = async (): Promise<boolean> => {
    if (!folder.subfolders.enabled || folder.subfolders.firstRunConfirmed) return true;
    if (!window.confirm(t('folderCard.advanced.firstRecursiveRun'))) return false;

    const confirmation = await ipc.folders.confirmFirstRun(folder.id);
    if (!confirmation.success) {
      setLastResult(readableIpcError(confirmation.error, t));
      return false;
    }
    await useAppStore.getState().loadFolders();
    return true;
  };

  const handleOrganize = async () => {
    if (undoBusy) return;
    if (busy) {
      if (activeSessionId) {
        setCancelRequested(true);
        await ipc.files.cancelOrganize(activeSessionId);
      }
      return;
    }

    if (!(await confirmDangerousImpact('manual'))) return;

    if (!(await ensureRecursiveConfirmation())) return;

    busyRef.current = true;
    activeSessionRef.current = null;
    setBusy(true);
    setCancelRequested(false);
    setProgress(null);
    setLastResult(null);

    try {
      const res = folder.subfolders.enabled
        ? await ipc.files.organizeRecursive(folder.path, {
            maxDepth: folder.subfolders.maxDepth,
            destinationMode: folder.subfolders.destinationMode,
            excludeFolders: folder.excludeFolders,
            excludePatterns: folder.excludePatterns,
          })
        : await ipc.files.organize(folder.path);
      if (res.success && res.data) {
        const { done, failed } = res.data.stats;
        if (res.data.cancelled) {
          setLastResult(t('folderCard.actions.cancelled', { count: done }));
        } else if (failed === 0) {
          setLastResult(t('dashboard.successOrganized', { count: done }));
        } else {
          setLastResult(t('dashboard.partialOrganized', { moved: done, failed }));
        }
        if (res.data.undoCount > 0) {
          const history = await ipc.files.listUndoSessions(folder.id);
          const sessions = history.success ? (history.data ?? []) : [];
          setUndoSession(sessions[0] ?? null);
          setUndoSessionCount(sessions.length);
        }
      } else if (res.error) {
        setLastResult(readableIpcError(res.error, t));
      }
    } finally {
      busyRef.current = false;
      activeSessionRef.current = null;
      setBusy(false);
      setActiveSessionId(null);
      setCancelRequested(false);
      setProgress(null);
    }
  };

  const handlePreview = async () => {
    setPreviewOpen(true);
    setPreviewLoading(true);
    setPreviewResult(null);
    setPreviewError(null);
    const response = await requestPreview();
    setPreviewLoading(false);
    if (response.success && response.data) {
      setPreviewResult(response.data);
    } else {
      setPreviewError(readableIpcError(response.error, t));
    }
  };

  const handleUndo = async () => {
    if (!undoSession || undoBusy) return;
    setUndoBusy(true);
    try {
      const res = await ipc.files.undoOrganize(undoSession.sessionId);
      if (res.data) {
        const summary = t('folderCard.actions.undone', {
          count: res.data.done,
          failed: res.data.failed,
        });
        setLastResult(
          res.success || !res.error ? summary : `${summary} ${readableIpcError(res.error, t)}`,
        );
      } else if (res.error) {
        setLastResult(readableIpcError(res.error, t));
      }
      const history = await ipc.files.listUndoSessions(folder.id);
      const sessions = history.success ? (history.data ?? []) : [];
      setUndoSession(sessions[0] ?? null);
      setUndoSessionCount(sessions.length);
    } finally {
      setUndoBusy(false);
    }
  };

  const handleDismissUndo = async () => {
    if (!undoSession || !window.confirm(t('folderCard.actions.dismissUndoConfirm'))) return;
    const response = await ipc.files.dismissUndo(undoSession.sessionId);
    if (!response.success) {
      setLastResult(readableIpcError(response.error, t));
      return;
    }
    const history = await ipc.files.listUndoSessions(folder.id);
    const sessions = history.success ? (history.data ?? []) : [];
    setUndoSession(sessions[0] ?? null);
    setUndoSessionCount(sessions.length);
  };

  const handleWatcherToggle = async () => {
    let response;
    if (folder.watcherStatus === 'running') {
      response = await ipc.watcher.pause(folder.id);
    } else if (folder.watcherStatus === 'paused') {
      response = await ipc.watcher.resume(folder.id);
    } else {
      response = await ipc.watcher.start(folder.id);
    }
    if (!response.success) setLastResult(readableIpcError(response.error, t));
    await useAppStore.getState().loadFolders();
  };

  const handleRecommendedRules = async () => {
    setInstallingRecommendations(true);
    const success = await applyRecommendedRules(folder.id);
    setInstallingRecommendations(false);
    if (success) setLastResult(t('folderCard.recommended.ready'));
  };

  const handleModeChange = async (mode: 'single' | 'auto') => {
    if (mode === folder.mode || modeChanging) return;
    setModeChanging(true);
    try {
      if (mode === 'auto' && folder.subfolders.enabled) {
        if (!(await ensureRecursiveConfirmation())) return;
      }
      if (mode === 'auto' && !(await confirmDangerousImpact('auto'))) return;
      const changed = await updateFolderMode(folder.id, mode);
      if (!changed) {
        setLastResult(readableIpcError(useAppStore.getState().error ?? undefined, t));
      }
    } finally {
      setModeChanging(false);
    }
  };

  const handleRename = async () => {
    const name = window.prompt(t('folderCard.rename'), folder.name)?.trim();
    if (name && name !== folder.name) {
      await updateFolderName(folder.id, name);
    }
  };

  const handleRemove = async () => {
    if (window.confirm(t('folderCard.removeConfirm', { name: folder.name }))) {
      await removeFolder(folder.id);
    }
  };

  const statusLabel =
    folder.mode === 'auto'
      ? t(`folderCard.watcher.${folder.watcherStatus}`)
      : t('folderCard.mode.single');
  const statusClass =
    folder.mode === 'auto' && folder.watcherStatus === 'running'
      ? 'badge-success'
      : folder.mode === 'auto' && folder.watcherStatus === 'paused'
        ? 'badge-warning'
        : 'bg-primary-700/30 text-primary-400';
  const formattedUndoAt = undoSession
    ? new Intl.DateTimeFormat(i18n.language, {
        dateStyle: 'short',
        timeStyle: 'short',
      }).format(new Date(undoSession.at))
    : null;

  return (
    <div className="card space-y-4">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0 flex-1">
          <h3 className="truncate text-base font-medium text-primary-100">{folder.name}</h3>
          <p className="mt-1 truncate text-xs text-primary-500" title={folder.path}>
            {folder.path}
          </p>
        </div>
        <span className={`badge shrink-0 ${statusClass}`}>{statusLabel}</span>
      </div>

      <div className="grid grid-cols-2 gap-1 rounded-md border border-primary-800 bg-primary-950 p-1 text-xs">
        {(['auto', 'single'] as const).map((mode) => (
          <button
            key={mode}
            onClick={() => void handleModeChange(mode)}
            disabled={modeChanging || busy || undoBusy}
            className={`rounded px-2 py-1.5 transition-colors ${
              folder.mode === mode
                ? 'bg-accent-600 text-white'
                : 'text-primary-400 hover:bg-primary-800'
            }`}
          >
            {t(`folderCard.mode.${mode}`)}
          </button>
        ))}
      </div>

      {folder.ruleIds.length === 0 ? (
        <button
          className="btn-secondary w-full"
          onClick={() => void handleRecommendedRules()}
          disabled={installingRecommendations || busy || undoBusy || modeChanging}
        >
          <Sparkles size={15} />
          {t(
            installingRecommendations
              ? 'folderCard.recommended.installing'
              : 'folderCard.recommended.button',
          )}
        </button>
      ) : (
        <div className="flex items-center justify-between text-xs">
          <span className="text-primary-400">
            {t('folderCard.recommended.active', { count: folder.ruleIds.length })}
          </span>
          <button onClick={onNavigateToRules} className="text-accent-400 hover:text-accent-300">
            {t('folderCard.rules')}
          </button>
        </div>
      )}

      <FolderAdvancedSettings folder={folder} />

      <div className="flex flex-wrap items-center gap-2 border-t border-primary-800 pt-3">
        <button
          className="btn-secondary"
          onClick={() => void handlePreview()}
          disabled={busy || undoBusy || modeChanging}
          title={t('folderCard.actions.preview')}
        >
          <Eye size={14} />
          {t('folderCard.actions.preview')}
        </button>
        <button
          className="btn-primary flex-1"
          onClick={() => void handleOrganize()}
          disabled={undoBusy || (busy && !activeSessionId) || modeChanging}
        >
          {busy ? <Square size={14} /> : <Play size={14} />}
          {busy
            ? t(cancelRequested ? 'folderCard.actions.cancelling' : 'folderCard.actions.cancel')
            : t('folderCard.actions.organize')}
        </button>
        {folder.mode === 'auto' && (
          <button
            className="btn-secondary"
            onClick={() => void handleWatcherToggle()}
            disabled={busy || undoBusy || modeChanging}
            title={t(
              folder.watcherStatus === 'running'
                ? 'folderCard.actions.pause'
                : folder.watcherStatus === 'paused'
                  ? 'folderCard.actions.resume'
                  : 'folderCard.actions.watch',
            )}
          >
            {folder.watcherStatus === 'running' ? <Pause size={14} /> : <Play size={14} />}
          </button>
        )}
        {undoSession && (
          <button
            className="btn-secondary"
            onClick={() => void handleUndo()}
            disabled={undoBusy || busy}
          >
            <RotateCcw size={14} />
            {t(undoBusy ? 'folderCard.actions.undoing' : 'folderCard.actions.undoLatest')}
          </button>
        )}
        {undoSession && (
          <button
            className="btn-ghost p-2"
            onClick={() => void handleDismissUndo()}
            disabled={undoBusy || busy}
            title={t('folderCard.actions.dismissUndo')}
          >
            <X size={14} />
          </button>
        )}
        <button
          className="btn-ghost"
          onClick={() => void handleRename()}
          title={t('folderCard.rename')}
          disabled={busy || undoBusy || modeChanging}
        >
          <Pencil size={14} />
        </button>
        <button
          className="btn-ghost"
          onClick={() => void handleRemove()}
          title={t('folderCard.actions.remove')}
          disabled={busy || undoBusy || modeChanging}
        >
          <Trash2 size={14} />
        </button>
      </div>

      {busy && progress && (
        <p className="text-xs text-primary-400">
          {progress.processed} / {progress.total}
        </p>
      )}
      {lastResult && <p className="text-xs text-primary-300">{lastResult}</p>}
      {undoSession && (
        <p className="text-xs text-primary-500">
          {t('folderCard.actions.undoHistory', {
            batches: undoSessionCount,
            count: undoSession.entryCount,
          })}
          {formattedUndoAt ? ` · ${formattedUndoAt}` : ''}
        </p>
      )}

      {previewOpen && (
        <PreviewDialog
          result={previewResult}
          loading={previewLoading}
          error={previewError}
          onClose={() => setPreviewOpen(false)}
        />
      )}
    </div>
  );
}

function PreviewDialog({
  result,
  loading,
  error,
  onClose,
}: {
  result: PreviewResult | null;
  loading: boolean;
  error: string | null;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const actionable = result?.plan.filter((item) => item.action) ?? [];
  const unmatched = (result?.plan.length ?? 0) - actionable.length;
  const groups = groupPreviewActions(actionable);
  const visibleGroups = groups.slice(0, 24);

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4">
      <section
        className="card max-h-[85vh] w-full max-w-3xl overflow-hidden p-0"
        role="dialog"
        aria-modal="true"
        aria-labelledby="folder-preview-title"
      >
        <header className="flex items-center justify-between border-b border-primary-800 px-5 py-4">
          <h2 id="folder-preview-title" className="text-lg font-semibold text-primary-50">
            {t('folderCard.preview.title')}
          </h2>
          <button className="btn-ghost p-2" onClick={onClose} aria-label={t('common.close')}>
            <X size={18} />
          </button>
        </header>

        <div className="max-h-[65vh] space-y-5 overflow-y-auto p-5">
          {loading && <p className="text-sm text-primary-400">{t('common.loading')}</p>}
          {error && <p className="rounded-md bg-red-500/10 p-3 text-sm text-red-300">{error}</p>}
          {result && (
            <>
              <div className="grid grid-cols-3 gap-2">
                <PreviewStat
                  value={actionable.length}
                  label={t('folderCard.preview.willOrganize')}
                  tone="accent"
                />
                <PreviewStat value={groups.length} label={t('folderCard.preview.categories')} />
                <PreviewStat value={unmatched} label={t('folderCard.preview.unchanged')} />
              </div>
              {result.stats.truncated && (
                <p className="rounded-md bg-yellow-500/10 p-3 text-xs text-yellow-200">
                  {t('folderCard.preview.truncated')}
                </p>
              )}
              {result.stats.errors > 0 && (
                <p className="rounded-md bg-yellow-500/10 p-3 text-xs text-yellow-200">
                  {t('folderCard.preview.scanErrors', { count: result.stats.errors })}
                </p>
              )}
              {actionable.length === 0 ? (
                <p className="py-8 text-center text-sm text-primary-500">
                  {t('folderCard.preview.noMatches')}
                </p>
              ) : (
                <div className="space-y-3">
                  <div>
                    <h3 className="text-sm font-medium text-primary-200">
                      {t('folderCard.preview.planTitle')}
                    </h3>
                    <p className="mt-1 text-xs text-primary-500">
                      {t('folderCard.preview.planHint')}
                    </p>
                  </div>
                  <div className="grid gap-3 sm:grid-cols-2">
                    {visibleGroups.map((group) => (
                      <PreviewGroupCard key={group.key} group={group} />
                    ))}
                  </div>
                </div>
              )}
              {groups.length > visibleGroups.length && (
                <p className="text-xs text-primary-500">
                  {t('folderCard.preview.groupsRemaining', {
                    count: groups.length - visibleGroups.length,
                  })}
                </p>
              )}
            </>
          )}
        </div>

        <footer className="flex justify-end border-t border-primary-800 px-5 py-4">
          <button className="btn-secondary" onClick={onClose}>
            {t('common.close')}
          </button>
        </footer>
      </section>
    </div>
  );
}

interface PreviewGroup {
  key: string;
  ruleName: string;
  action: Action;
  files: PlannedAction['file'][];
}

function groupPreviewActions(items: PlannedAction[]): PreviewGroup[] {
  const groups = new Map<string, PreviewGroup>();
  for (const item of items) {
    if (!item.action) continue;
    const destination = item.action.type === 'move' ? item.action.destination : '';
    const key = `${item.ruleId ?? item.ruleName ?? 'rule'}:${item.action.type}:${destination}`;
    const existing = groups.get(key);
    if (existing) {
      existing.files.push(item.file);
    } else {
      groups.set(key, {
        key,
        ruleName: item.ruleName ?? item.file.extension,
        action: item.action,
        files: [item.file],
      });
    }
  }
  return [...groups.values()].sort((left, right) => right.files.length - left.files.length);
}

function PreviewStat({
  value,
  label,
  tone = 'neutral',
}: {
  value: number;
  label: string;
  tone?: 'accent' | 'neutral';
}) {
  return (
    <div className="rounded-lg border border-primary-800 bg-primary-950/70 p-3 text-center">
      <p
        className={`text-xl font-semibold ${tone === 'accent' ? 'text-accent-400' : 'text-primary-200'}`}
      >
        {value.toLocaleString()}
      </p>
      <p className="mt-1 text-[11px] leading-tight text-primary-500">{label}</p>
    </div>
  );
}

function PreviewGroupCard({ group }: { group: PreviewGroup }) {
  const { t } = useTranslation();
  const dangerous = group.action.type === 'delete' || group.action.type === 'runCommand';
  const Icon =
    group.action.type === 'move'
      ? FolderIcon
      : group.action.type === 'trash'
        ? Trash2
        : group.action.type === 'delete'
          ? AlertTriangle
          : Terminal;
  const destination =
    group.action.type === 'move' ? friendlyDestination(group.action.destination) : null;
  const examples = group.files.slice(0, 3);

  return (
    <article
      className={`rounded-lg border p-3 ${
        dangerous ? 'border-red-500/35 bg-red-500/5' : 'border-primary-800 bg-primary-950/45'
      }`}
    >
      <div className="flex items-start gap-3">
        <span
          className={`rounded-lg p-2 ${
            dangerous ? 'bg-red-500/10 text-red-300' : 'bg-accent-500/10 text-accent-400'
          }`}
        >
          <Icon size={18} />
        </span>
        <div className="min-w-0 flex-1">
          <div className="flex items-start justify-between gap-2">
            <div className="min-w-0">
              <p className="truncate text-sm font-medium text-primary-200">{group.ruleName}</p>
              <p className={`text-xs ${dangerous ? 'text-red-300' : 'text-primary-500'}`}>
                {t(`rules.action.${group.action.type}`)}
              </p>
            </div>
            <span className="badge bg-primary-800 text-primary-300">
              {group.files.length.toLocaleString()}
            </span>
          </div>
          {destination && (
            <p
              className="mt-2 flex items-center gap-1.5 truncate text-xs text-accent-300"
              title={group.action.type === 'move' ? group.action.destination : undefined}
            >
              <FolderIcon size={12} /> {destination}
            </p>
          )}
        </div>
      </div>
      <div className="mt-3 flex flex-wrap gap-1.5">
        {examples.map((file) => (
          <span
            key={file.path}
            className="inline-flex max-w-full items-center gap-1 rounded bg-primary-800/70 px-2 py-1 text-[11px] text-primary-400"
            title={file.path}
          >
            <Files size={11} className="shrink-0" />
            <span className="truncate">{file.name}</span>
          </span>
        ))}
        {group.files.length > examples.length && (
          <span className="px-1 py-1 text-[11px] text-primary-500">
            {t('folderCard.preview.moreFiles', { count: group.files.length - examples.length })}
          </span>
        )}
      </div>
    </article>
  );
}

function friendlyDestination(destination: string): string {
  const parts = destination.split(/[\\/]/).filter(Boolean);
  return parts.slice(-2).join(' / ') || destination;
}
