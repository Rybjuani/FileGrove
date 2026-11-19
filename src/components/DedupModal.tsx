/**
 * DedupModal — Safe, visual file deduplication dialog.
 *
 * Flow:
 *  1. User clicks "Buscar duplicados" on a FolderCard.
 *  2. Modal opens, calls `ipc.dedup.findDuplicates` and shows a spinner.
 *  3. Results are listed grouped by hash: original (highlighted) + duplicates (checkboxes).
 *  4. User selects which duplicates to trash (all pre-selected by default).
 *  5. "Mover a la Papelera" button shows a confirmation, then calls `ipc.dedup.trashDuplicates`.
 *  6. Result summary is shown. No permanent deletion ever happens here.
 */

import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Copy, Shield, Star, Trash2, X } from 'lucide-react';
import { ipc } from '../ipc';
import type { DuplicateFile, DuplicateGroup, DuplicateReport } from '../ipc/types';
import { useDialogFocus } from '../lib/dialog';

// ─── Helpers ─────────────────────────────────────────────────────────────────

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

function shortPath(path: string): string {
  // Show only the last 2 path segments to keep the UI clean.
  const sep = path.includes('\\') ? '\\' : '/';
  const parts = path.split(sep).filter(Boolean);
  return parts.slice(-2).join(sep) || path;
}

// ─── DedupModal ──────────────────────────────────────────────────────────────

interface DedupModalProps {
  folderPath: string;
  excludeFolders: string[];
  excludePatterns: string[];
  onClose: () => void;
}

export function DedupModal({
  folderPath,
  excludeFolders,
  excludePatterns,
  onClose,
}: DedupModalProps) {
  const { t } = useTranslation();
  const dialogRef = useDialogFocus<HTMLElement>(onClose);

  const [scanning, setScanning] = useState(true);
  const [report, setReport] = useState<DuplicateReport | null>(null);
  const [scanError, setScanError] = useState<string | null>(null);
  const [scanNonce, setScanNonce] = useState(0);
  // Set of duplicate paths currently selected for trashing
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [trashing, setTrashing] = useState(false);
  const [trashResult, setTrashResult] = useState<string | null>(null);
  const [trashError, setTrashError] = useState<string | null>(null);

  // Run scan on mount
  useEffect(() => {
    let disposed = false;
    setScanning(true);
    setScanError(null);
    setReport(null);
    ipc.dedup
      .findDuplicates(folderPath, excludeFolders, excludePatterns)
      .then((res) => {
        if (disposed) return;
        if (res.success && res.data) {
          setReport(res.data);
          // Pre-select ALL duplicates by default (originals are excluded)
          const allDups = new Set<string>();
          for (const group of res.data.groups) {
            for (const dup of group.duplicates) {
              allDups.add(dup.path);
            }
          }
          setSelected(allDups);
        } else {
          setScanError(res.error ?? t('errors.generic'));
        }
      })
      .catch((e: unknown) => {
        if (!disposed) setScanError(e instanceof Error ? e.message : t('errors.generic'));
      })
      .finally(() => {
        if (!disposed) setScanning(false);
      });
    return () => {
      disposed = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [folderPath, scanNonce]);

  const allDuplicatePaths = report?.groups.flatMap((g) => g.duplicates.map((d) => d.path)) ?? [];

  const togglePath = (path: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  };

  const selectAll = () => setSelected(new Set(allDuplicatePaths));
  const deselectAll = () => setSelected(new Set());

  const handleTrash = async () => {
    const paths = [...selected];
    if (paths.length === 0) return;
    if (!window.confirm(t('dedup.trashConfirm', { count: paths.length }))) return;

    setTrashing(true);
    setTrashResult(null);
    setTrashError(null);
    try {
      const res = await ipc.dedup.trashDuplicates(paths);
      if (!res.success || !res.data) {
        setTrashError(res.error ?? t('dedup.trashError'));
        return;
      }

      const { trashed, failed } = res.data;
      if (failed > 0) {
        setTrashResult(t('dedup.trashPartial', { trashed, failed }));
        // Re-scan so the report only shows files that still exist.
        setScanNonce((nonce) => nonce + 1);
        return;
      }

      setTrashResult(t('dedup.trashDone', { trashed }));
      // Remove successfully trashed paths from the report.
      setReport((prev) => {
        if (!prev) return prev;
        const trashedSet = new Set(paths);
        const newGroups = prev.groups
          .map((g) => ({
            ...g,
            duplicates: g.duplicates.filter((d) => !trashedSet.has(d.path)),
          }))
          .filter((g) => g.duplicates.length > 0);
        const totalDuplicates = newGroups.reduce((acc, g) => acc + g.duplicates.length, 0);
        const totalWastedBytes = newGroups.reduce(
          (acc, g) => acc + g.size * g.duplicates.length,
          0,
        );
        return { groups: newGroups, totalDuplicates, totalWastedBytes };
      });
      setSelected(new Set());
    } catch (error) {
      setTrashError(error instanceof Error ? error.message : t('errors.generic'));
    } finally {
      setTrashing(false);
    }
  };

  const wastedLabel = report ? formatBytes(report.totalWastedBytes) : '';

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4">
      <section
        ref={dialogRef}
        className="card flex max-h-[90vh] w-full max-w-2xl flex-col overflow-hidden p-0"
        role="dialog"
        aria-modal="true"
        aria-labelledby="dedup-modal-title"
        tabIndex={-1}
      >
        {/* Header */}
        <header className="flex shrink-0 items-center justify-between border-b border-primary-800 px-5 py-4">
          <div className="flex items-center gap-3">
            <span className="rounded-lg bg-green-500/10 p-2 text-green-400">
              <Copy size={18} aria-hidden="true" />
            </span>
            <h2 id="dedup-modal-title" className="text-lg font-semibold text-primary-50">
              {t('dedup.title')}
            </h2>
          </div>
          <button className="btn-ghost p-2" onClick={onClose} aria-label={t('common.close')}>
            <X size={18} />
          </button>
        </header>

        {/* Body */}
        <div className="min-h-0 flex-1 overflow-y-auto p-5 space-y-4">
          {/* Scanning spinner */}
          {scanning && (
            <p className="py-10 text-center text-sm text-primary-400" role="status">
              {t('dedup.scanning')}
            </p>
          )}

          {/* Scan error */}
          {scanError && (
            <p className="rounded-md bg-red-500/10 p-3 text-sm text-red-300" role="alert">
              {scanError}
            </p>
          )}

          {/* No results */}
          {!scanning && report && report.groups.length === 0 && (
            <div className="flex flex-col items-center gap-3 py-10 text-center">
              <span className="rounded-full bg-green-500/10 p-4 text-green-400">
                <Shield size={32} aria-hidden="true" />
              </span>
              <p className="text-sm text-primary-400">{t('dedup.noResults')}</p>
            </div>
          )}

          {/* Results */}
          {!scanning && report && report.groups.length > 0 && (
            <>
              {/* Summary bar */}
              <div className="flex items-center justify-between gap-2 rounded-lg border border-primary-800 bg-primary-950/60 px-4 py-3 text-sm">
                <span className="text-primary-300">
                  {t('dedup.summary', {
                    count: report.groups.length,
                    wasted: wastedLabel,
                    defaultValue_one: `${report.groups.length} grupo · ${wastedLabel} desperdiciados`,
                    defaultValue_other: `${report.groups.length} grupos · ${wastedLabel} desperdiciados`,
                  })}
                </span>
                <div className="flex gap-2">
                  <button
                    className="text-xs text-accent-400 hover:text-accent-300"
                    onClick={selectAll}
                    disabled={trashing}
                  >
                    {t('dedup.selectAll')}
                  </button>
                  <span className="text-primary-700">·</span>
                  <button
                    className="text-xs text-primary-400 hover:text-primary-300"
                    onClick={deselectAll}
                    disabled={trashing}
                  >
                    {t('dedup.deselectAll')}
                  </button>
                </div>
              </div>

              {/* Groups */}
              <div className="space-y-3">
                {report.groups.map((group) => (
                  <DuplicateGroupCard
                    key={group.hash}
                    group={group}
                    selected={selected}
                    onToggle={togglePath}
                    disabled={trashing}
                  />
                ))}
              </div>
            </>
          )}

          {/* Trash result feedback */}
          {trashResult && (
            <p
              className="rounded-md bg-green-500/10 p-3 text-sm text-green-300"
              role="status"
              aria-live="polite"
            >
              {trashResult}
            </p>
          )}
          {trashError && (
            <p className="rounded-md bg-red-500/10 p-3 text-sm text-red-300" role="alert">
              {trashError}
            </p>
          )}
        </div>

        {/* Footer */}
        <footer className="flex shrink-0 items-center justify-between gap-3 border-t border-primary-800 px-5 py-4">
          <p className="text-xs text-primary-500">
            🛡️ {/* safe icon */}
            {selected.size > 0 ? t('dedup.trashHint', { count: selected.size }) : ''}
          </p>
          <div className="flex gap-2">
            <button className="btn-secondary" onClick={onClose} disabled={trashing}>
              {t('dedup.close')}
            </button>
            {report && report.groups.length > 0 && (
              <button
                className="btn-primary"
                onClick={() => void handleTrash()}
                disabled={trashing || selected.size === 0}
              >
                <Trash2 size={14} />
                {trashing ? t('common.loading') : t('dedup.trashSelected')}
              </button>
            )}
          </div>
        </footer>
      </section>
    </div>
  );
}

// ─── DuplicateGroupCard ───────────────────────────────────────────────────────

function DuplicateGroupCard({
  group,
  selected,
  onToggle,
  disabled,
}: {
  group: DuplicateGroup;
  selected: Set<string>;
  onToggle: (path: string) => void;
  disabled: boolean;
}) {
  const { t } = useTranslation();

  return (
    <div className="rounded-lg border border-primary-800 bg-primary-950/40 p-3 space-y-2">
      <div className="flex items-center gap-2 text-xs text-primary-500">
        <span>{formatBytes(group.size)}</span>
        <span>·</span>
        <span>{t('dedup.identicalFiles', { count: group.duplicates.length + 1 })}</span>
      </div>

      {/* Original */}
      <FileRow file={group.original} isOriginal label={t('dedup.original')} />

      {/* Duplicates with checkboxes */}
      {group.duplicates.map((dup) => (
        <FileRow
          key={dup.path}
          file={dup}
          isOriginal={false}
          label={t('dedup.duplicate')}
          checked={selected.has(dup.path)}
          onToggle={() => onToggle(dup.path)}
          disabled={disabled}
        />
      ))}
    </div>
  );
}

// ─── FileRow ─────────────────────────────────────────────────────────────────

function FileRow({
  file,
  isOriginal,
  label,
  checked,
  onToggle,
  disabled,
}: {
  file: DuplicateFile;
  isOriginal: boolean;
  label: string;
  checked?: boolean;
  onToggle?: () => void;
  disabled?: boolean;
}) {
  const { t } = useTranslation();

  return (
    <label
      className={`flex items-center gap-3 rounded-md px-2 py-1.5 transition-colors ${
        isOriginal ? 'bg-green-500/8 cursor-default' : 'cursor-pointer hover:bg-primary-800/40'
      }`}
    >
      {isOriginal ? (
        <span className="shrink-0 text-green-400" title={label}>
          <Star size={14} aria-label={label} />
        </span>
      ) : (
        <input
          type="checkbox"
          checked={checked ?? false}
          onChange={onToggle}
          disabled={disabled}
          className="shrink-0 accent-accent-500"
          aria-label={t('dedup.selectDuplicate', { name: file.name })}
        />
      )}
      <span className="min-w-0 flex-1">
        <span className="block truncate text-sm font-medium text-primary-200" title={file.path}>
          {file.name}
        </span>
        <span className="block truncate text-[11px] text-primary-500" title={file.path}>
          {shortPath(file.path)}
        </span>
      </span>
      {isOriginal && (
        <span className="shrink-0 rounded-full bg-green-500/15 px-2 py-0.5 text-[10px] font-medium text-green-400">
          {label}
        </span>
      )}
    </label>
  );
}
