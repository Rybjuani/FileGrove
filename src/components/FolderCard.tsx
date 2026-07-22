import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Pencil, Play, Square, Trash2 } from 'lucide-react';
import { useAppStore } from '../stores/useAppStore';
import { ipc } from '../ipc';
import type { Folder } from '../ipc/types';

interface Props {
  folder: Folder;
  onNavigateToRules: () => void;
}

export function FolderCard({ folder, onNavigateToRules }: Props) {
  const { t } = useTranslation();
  const { removeFolder, updateFolderName, updateFolderMode } = useAppStore();
  const [busy, setBusy] = useState(false);
  const [lastResult, setLastResult] = useState<string | null>(null);

  // Subscribe to watcher status broadcasts
  useEffect(() => {
    const off = ipc.watcher.onStatusChanged((data) => {
      if (data.folderId === folder.id) {
        // The store will reload via the next list() call; for now, just log.
        console.debug('watcher status', data);
      }
    });
    return () => void off.then((fn) => fn());
  }, [folder.id]);

  const handleOrganize = async () => {
    setBusy(true);
    setLastResult(null);
    const res = await ipc.files.organize(folder.path);
    setBusy(false);
    if (res.success && res.data) {
      const { done, failed } = res.data.stats;
      if (failed === 0) {
        setLastResult(t('dashboard.successOrganized', { count: done }));
      } else {
        setLastResult(t('dashboard.partialOrganized', { moved: done, failed }));
      }
    } else if (res.error) {
      setLastResult(res.error);
    }
    setTimeout(() => setLastResult(null), 5000);
  };

  const handleWatcherToggle = async () => {
    if (folder.watcherStatus === 'running') {
      await ipc.watcher.stop(folder.id);
    } else {
      await ipc.watcher.start(folder.id);
    }
    await useAppStore.getState().loadFolders();
  };

  const handleModeChange = async (mode: 'single' | 'auto' | 'scheduled') => {
    await updateFolderMode(folder.id, mode);
  };

  const handleRename = async () => {
    const name = window.prompt(t('folderCard.rename'), folder.name)?.trim();
    if (name && name !== folder.name) {
      await updateFolderName(folder.id, name);
    }
  };

  return (
    <div className="card space-y-4">
      <div className="flex items-start justify-between">
        <div className="min-w-0 flex-1">
          <h3 className="truncate text-base font-medium text-primary-100">{folder.name}</h3>
          <p className="mt-1 truncate text-xs text-primary-500" title={folder.path}>
            {folder.path}
          </p>
        </div>
        <span
          className={`badge ${
            folder.watcherStatus === 'running'
              ? 'badge-success'
              : folder.watcherStatus === 'paused'
                ? 'badge-warning'
                : 'bg-primary-700/30 text-primary-400'
          }`}
        >
          {t(`folderCard.watcher.${folder.watcherStatus}`)}
        </span>
      </div>

      {/* Mode selector */}
      <div className="grid grid-cols-3 gap-1 rounded-md border border-primary-800 bg-primary-950 p-1 text-xs">
        {(['single', 'auto', 'scheduled'] as const).map((mode) => (
          <button
            key={mode}
            onClick={() => void handleModeChange(mode)}
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

      {/* Rules summary */}
      <div className="flex items-center justify-between text-xs">
        <span className="text-primary-400">
          {folder.ruleIds.length > 0
            ? t('folderCard.rules') + ': ' + folder.ruleIds.length
            : t('folderCard.noRules')}
        </span>
        <button onClick={onNavigateToRules} className="text-accent-400 hover:text-accent-300">
          {folder.ruleIds.length > 0 ? t('folderCard.rules') : t('folderCard.addRules')}
        </button>
      </div>

      {/* Actions */}
      <div className="flex flex-wrap items-center gap-2 border-t border-primary-800 pt-3">
        <button
          className="btn-primary flex-1"
          onClick={() => void handleOrganize()}
          disabled={busy}
        >
          {busy ? <Square size={14} /> : <Play size={14} />}
          {busy ? t('folderCard.actions.organizing') : t('folderCard.actions.organize')}
        </button>
        {folder.mode !== 'single' && (
          <button
            className="btn-secondary"
            onClick={() => void handleWatcherToggle()}
            title={
              folder.watcherStatus === 'running'
                ? t('folderCard.actions.stop')
                : t('folderCard.actions.watch')
            }
          >
            {folder.watcherStatus === 'running' ? <Square size={14} /> : <Play size={14} />}
          </button>
        )}
        <button
          className="btn-ghost"
          onClick={() => void handleRename()}
          title={t('folderCard.rename')}
        >
          <Pencil size={14} />
        </button>
        <button
          className="btn-ghost"
          onClick={() => void removeFolder(folder.id)}
          title={t('folderCard.actions.remove')}
        >
          <Trash2 size={14} />
        </button>
      </div>

      {lastResult && <p className="text-xs text-primary-300">{lastResult}</p>}
    </div>
  );
}
