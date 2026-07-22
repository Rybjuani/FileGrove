import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { CalendarClock, Pause, Pencil, Play, RotateCcw, Square, Trash2 } from 'lucide-react';
import { useAppStore } from '../stores/useAppStore';
import { ipc } from '../ipc';
import type { Folder, OrganizeProgress, Schedule } from '../ipc/types';

interface Props {
  folder: Folder;
  onNavigateToRules: () => void;
}

const DEFAULT_SCHEDULE: Schedule = {
  enabled: false,
  daysOfWeek: [1, 2, 3, 4, 5],
  time: '09:00',
};

const DAYS = ['sun', 'mon', 'tue', 'wed', 'thu', 'fri', 'sat'] as const;

export function FolderCard({ folder, onNavigateToRules }: Props) {
  const { t, i18n } = useTranslation();
  const { removeFolder, updateFolderName, updateFolderMode, updateFolderSchedule } = useAppStore();
  const schedule = folder.schedule ?? DEFAULT_SCHEDULE;
  const [busy, setBusy] = useState(false);
  const [activeSessionId, setActiveSessionId] = useState<string | null>(null);
  const [cancelRequested, setCancelRequested] = useState(false);
  const [progress, setProgress] = useState<OrganizeProgress | null>(null);
  const [undoSessionId, setUndoSessionId] = useState<string | null>(null);
  const [lastResult, setLastResult] = useState<string | null>(null);
  const [nextRun, setNextRun] = useState<string | null>(null);
  const [scheduleTime, setScheduleTime] = useState(schedule.time);
  const [scheduleSaving, setScheduleSaving] = useState(false);
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

  useEffect(() => {
    setScheduleTime(schedule.time);
  }, [schedule.time]);

  useEffect(() => {
    let disposed = false;
    if (folder.mode !== 'scheduled' || !schedule.enabled) {
      setNextRun(null);
      return;
    }
    const loadNextRun = () => {
      void ipc.folders.getNextRun(folder.id).then((res) => {
        if (!disposed && res.success && res.data) {
          setNextRun(res.data.nextRun);
        }
      });
    };
    loadNextRun();
    const timer = window.setInterval(loadNextRun, 60_000);
    return () => {
      disposed = true;
      window.clearInterval(timer);
    };
  }, [folder.id, folder.mode, folder.schedule, schedule.enabled]);

  const handleOrganize = async () => {
    if (busy) {
      if (activeSessionId) {
        setCancelRequested(true);
        await ipc.files.cancelOrganize(activeSessionId);
      }
      return;
    }

    busyRef.current = true;
    activeSessionRef.current = null;
    setBusy(true);
    setCancelRequested(false);
    setProgress(null);
    setUndoSessionId(null);
    setLastResult(null);

    try {
      const res = await ipc.files.organize(folder.path);
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
          setUndoSessionId(res.data.sessionId);
        }
      } else if (res.error) {
        setLastResult(res.error);
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

  const handleUndo = async () => {
    if (!undoSessionId) return;
    const res = await ipc.files.undoOrganize(undoSessionId);
    if (res.success && res.data) {
      setLastResult(
        t('folderCard.actions.undone', {
          count: res.data.done,
          failed: res.data.failed,
        }),
      );
      setUndoSessionId(null);
    } else if (res.error) {
      setLastResult(res.error);
    }
  };

  const handleWatcherToggle = async () => {
    if (folder.watcherStatus === 'running') {
      await ipc.watcher.pause(folder.id);
    } else if (folder.watcherStatus === 'paused') {
      await ipc.watcher.resume(folder.id);
    } else {
      await ipc.watcher.start(folder.id);
    }
    await useAppStore.getState().loadFolders();
  };

  const handleModeChange = async (mode: 'single' | 'auto' | 'scheduled') => {
    if (mode !== folder.mode) {
      await updateFolderMode(folder.id, mode);
    }
  };

  const saveSchedule = async (nextSchedule: Schedule) => {
    setScheduleSaving(true);
    await updateFolderSchedule(folder.id, nextSchedule);
    setScheduleSaving(false);
  };

  const toggleScheduleDay = async (day: number) => {
    const daysOfWeek = schedule.daysOfWeek.includes(day)
      ? schedule.daysOfWeek.filter((value) => value !== day)
      : [...schedule.daysOfWeek, day].sort((a, b) => a - b);
    if (daysOfWeek.length === 0) return;
    await saveSchedule({ ...schedule, daysOfWeek });
  };

  const saveScheduleTime = async () => {
    if (scheduleTime !== schedule.time && /^([01]\d|2[0-3]):[0-5]\d$/.test(scheduleTime)) {
      await saveSchedule({ ...schedule, time: scheduleTime });
    } else {
      setScheduleTime(schedule.time);
    }
  };

  const handleRename = async () => {
    const name = window.prompt(t('folderCard.rename'), folder.name)?.trim();
    if (name && name !== folder.name) {
      await updateFolderName(folder.id, name);
    }
  };

  const statusLabel =
    folder.mode === 'auto'
      ? t(`folderCard.watcher.${folder.watcherStatus}`)
      : folder.mode === 'scheduled'
        ? t(
            schedule.enabled
              ? 'folderCard.schedule.statusActive'
              : 'folderCard.schedule.statusDisabled',
          )
        : t('folderCard.mode.single');
  const statusClass =
    (folder.mode === 'auto' && folder.watcherStatus === 'running') ||
    (folder.mode === 'scheduled' && schedule.enabled)
      ? 'badge-success'
      : folder.mode === 'auto' && folder.watcherStatus === 'paused'
        ? 'badge-warning'
        : 'bg-primary-700/30 text-primary-400';
  const formattedNextRun = nextRun
    ? new Intl.DateTimeFormat(i18n.language, {
        weekday: 'short',
        hour: '2-digit',
        minute: '2-digit',
      }).format(new Date(nextRun))
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

      {folder.mode === 'scheduled' && (
        <div className="space-y-3 rounded-md border border-primary-800 bg-primary-950/60 p-3">
          <div className="flex items-center justify-between">
            <span className="flex items-center gap-2 text-xs font-medium text-primary-200">
              <CalendarClock size={14} />
              {t('folderCard.schedule.title')}
            </span>
            <button
              className={`rounded-full px-2 py-1 text-xs ${
                schedule.enabled
                  ? 'bg-accent-500/15 text-accent-400'
                  : 'bg-primary-800 text-primary-400'
              }`}
              onClick={() => void saveSchedule({ ...schedule, enabled: !schedule.enabled })}
              disabled={scheduleSaving}
            >
              {t(schedule.enabled ? 'folderCard.schedule.enabled' : 'folderCard.schedule.disabled')}
            </button>
          </div>

          <div>
            <p className="mb-2 text-xs text-primary-500">{t('folderCard.schedule.days')}</p>
            <div className="grid grid-cols-7 gap-1">
              {DAYS.map((day, index) => (
                <button
                  key={day}
                  className={`rounded py-1 text-[11px] ${
                    schedule.daysOfWeek.includes(index)
                      ? 'bg-accent-600 text-white'
                      : 'bg-primary-800 text-primary-400'
                  }`}
                  onClick={() => void toggleScheduleDay(index)}
                  disabled={scheduleSaving}
                  title={t(`folderCard.schedule.day.${day}.long`)}
                >
                  {t(`folderCard.schedule.day.${day}.short`)}
                </button>
              ))}
            </div>
          </div>

          <div className="flex items-center justify-between gap-3">
            <label className="text-xs text-primary-500" htmlFor={`schedule-${folder.id}`}>
              {t('folderCard.schedule.time')}
            </label>
            <input
              id={`schedule-${folder.id}`}
              type="time"
              className="input w-28 py-1"
              value={scheduleTime}
              onChange={(event) => setScheduleTime(event.target.value)}
              onBlur={() => void saveScheduleTime()}
              disabled={scheduleSaving}
            />
          </div>

          <p className="text-xs text-primary-400">
            {t('folderCard.schedule.nextRunLabel')}:&nbsp;
            <span className="text-primary-200">
              {formattedNextRun ?? t('folderCard.schedule.notScheduled')}
            </span>
          </p>
        </div>
      )}

      <div className="flex items-center justify-between text-xs">
        <span className="text-primary-400">
          {folder.ruleIds.length > 0
            ? `${t('folderCard.rules')}: ${folder.ruleIds.length}`
            : t('folderCard.noRules')}
        </span>
        <button onClick={onNavigateToRules} className="text-accent-400 hover:text-accent-300">
          {folder.ruleIds.length > 0 ? t('folderCard.rules') : t('folderCard.addRules')}
        </button>
      </div>

      <div className="flex flex-wrap items-center gap-2 border-t border-primary-800 pt-3">
        <button
          className="btn-primary flex-1"
          onClick={() => void handleOrganize()}
          disabled={busy && !activeSessionId}
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
        {undoSessionId && (
          <button className="btn-secondary" onClick={() => void handleUndo()}>
            <RotateCcw size={14} />
            {t('folderCard.actions.undo')}
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
          disabled={busy}
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
    </div>
  );
}
