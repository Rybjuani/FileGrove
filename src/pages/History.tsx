import { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ChevronDown, ChevronLeft, ChevronRight, ChevronUp, Trash2 } from 'lucide-react';
import { ipc } from '../ipc';
import type { ActivityEntry, ActivityType } from '../ipc/types';
import { readableIpcError } from '../lib/errors';

interface Props {
  onNavigateToRules: () => void;
}

type StatusFilter = 'all' | 'success' | 'failure';
type TypeFilter = 'all' | ActivityType;

export function History({ onNavigateToRules }: Props) {
  const { t, i18n } = useTranslation();
  const [items, setItems] = useState<ActivityEntry[]>([]);
  const [total, setTotal] = useState(0);
  const [status, setStatus] = useState<StatusFilter>('all');
  const [type, setType] = useState<TypeFilter>('all');
  const [page, setPage] = useState(0);
  const [perPage, setPerPage] = useState(20);
  const [expanded, setExpanded] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const pages = Math.max(1, Math.ceil(total / perPage));
  const filtersActive = status !== 'all' || type !== 'all';

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    const response = await ipc.activity.list({
      limit: perPage,
      offset: page * perPage,
      ...(status === 'all' ? {} : { success: status === 'success' }),
      ...(type === 'all' ? {} : { type }),
    });
    if (response.success && response.data) {
      setItems(response.data.items);
      setTotal(response.data.total);
    } else {
      setError(readableIpcError(response.error, t));
    }
    setLoading(false);
  }, [page, perPage, status, t, type]);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    let timer: number | undefined;
    const unlisten = ipc.activity.onChanged(() => {
      window.clearTimeout(timer);
      timer = window.setTimeout(() => void load(), 250);
    });
    return () => {
      window.clearTimeout(timer);
      void unlisten.then((stop) => stop());
    };
  }, [load]);

  useEffect(() => {
    setPage(0);
    setExpanded(null);
  }, [status, type, perPage]);

  useEffect(() => {
    if (page >= pages) setPage(Math.max(0, pages - 1));
  }, [page, pages]);

  const typeOptions = useMemo<TypeFilter[]>(
    () => ['all', 'move', 'trash', 'delete', 'runCommand'],
    [],
  );

  const clear = async () => {
    if (!window.confirm(t('history.clearConfirm'))) return;
    const response = await ipc.activity.clear();
    if (response.success) {
      setPage(0);
      setExpanded(null);
      await load();
    } else {
      setError(readableIpcError(response.error, t));
    }
  };

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <h1 className="text-2xl font-semibold text-primary-50">{t('history.title')}</h1>
          <p className="mt-1 text-sm text-primary-400">{t('history.subtitle')}</p>
        </div>
        <button
          className="btn-ghost text-red-400"
          onClick={() => void clear()}
          disabled={total === 0}
        >
          <Trash2 size={14} />
          {t('history.clearAll')}
        </button>
      </div>

      <div className="card grid gap-4 sm:grid-cols-2">
        <label className="text-xs font-medium text-primary-400">
          {t('history.statusFilter')}
          <select
            className="input mt-1"
            value={status}
            onChange={(event) => setStatus(event.target.value as StatusFilter)}
          >
            <option value="all">{t('history.filterAll')}</option>
            <option value="success">{t('history.filterSuccess')}</option>
            <option value="failure">{t('history.filterFailure')}</option>
          </select>
        </label>
        <label className="text-xs font-medium text-primary-400">
          {t('history.typeFilter')}
          <select
            className="input mt-1"
            value={type}
            onChange={(event) => setType(event.target.value as TypeFilter)}
          >
            {typeOptions.map((value) => (
              <option key={value} value={value}>
                {value === 'all' ? t('history.allTypes') : t(`activity.type.${value}`)}
              </option>
            ))}
          </select>
        </label>
      </div>

      {error && (
        <p
          role="alert"
          className="rounded-md border border-red-500/30 bg-red-500/10 p-3 text-sm text-red-300"
        >
          {error}
        </p>
      )}

      {loading && items.length === 0 ? (
        <div className="card py-12 text-center text-sm text-primary-400">{t('common.loading')}</div>
      ) : items.length === 0 ? (
        <div className="card py-12 text-center">
          <p className="text-sm text-primary-400">
            {filtersActive ? t('activity.noMatches') : t('history.empty')}
          </p>
          {!filtersActive && (
            <p className="mt-2 text-xs text-primary-500">{t('history.emptyDesc')}</p>
          )}
          {filtersActive && (
            <button
              className="btn-secondary mt-4"
              onClick={() => {
                setStatus('all');
                setType('all');
              }}
            >
              {t('activity.clearFilters')}
            </button>
          )}
          {!filtersActive && (
            <button className="btn-secondary mt-4" onClick={onNavigateToRules}>
              {t('activity.emptyCta')}
            </button>
          )}
        </div>
      ) : (
        <div className="card overflow-hidden p-0">
          <div className="overflow-x-auto">
            <table className="w-full min-w-[760px] text-left text-sm">
              <thead className="border-b border-primary-800 bg-primary-950/50 text-xs uppercase text-primary-500">
                <tr>
                  <th className="w-9 px-3 py-3" aria-label={t('history.details')} />
                  <th className="px-3 py-3">{t('activity.file')}</th>
                  <th className="px-3 py-3">{t('activity.ruleLabel')}</th>
                  <th className="px-3 py-3">{t('history.type')}</th>
                  <th className="px-3 py-3">{t('history.when')}</th>
                  <th className="px-3 py-3 text-right">{t('history.status')}</th>
                </tr>
              </thead>
              <tbody>
                {items.map((item) => {
                  const isExpanded = expanded === item.id;
                  return (
                    <HistoryRow
                      key={item.id}
                      item={item}
                      expanded={isExpanded}
                      locale={i18n.language}
                      onToggle={() => setExpanded(isExpanded ? null : item.id)}
                    />
                  );
                })}
              </tbody>
            </table>
          </div>

          <footer className="flex flex-wrap items-center justify-between gap-3 border-t border-primary-800 px-4 py-3">
            <p className="text-xs text-primary-500">{t('activity.total', { count: total })}</p>
            <div className="flex items-center gap-3">
              <label className="flex items-center gap-2 text-xs text-primary-500">
                {t('activity.perPage')}
                <select
                  className="input w-20 py-1"
                  value={perPage}
                  onChange={(event) => setPerPage(Number(event.target.value))}
                >
                  {[20, 50, 100].map((value) => (
                    <option key={value}>{value}</option>
                  ))}
                </select>
              </label>
              <span className="text-xs text-primary-400">
                {t('activity.pageOf', { page: page + 1, pages })}
              </span>
              <button
                className="btn-ghost p-2"
                onClick={() => setPage((current) => Math.max(0, current - 1))}
                disabled={page === 0 || loading}
                aria-label={t('activity.prevPage')}
              >
                <ChevronLeft size={16} />
              </button>
              <button
                className="btn-ghost p-2"
                onClick={() => setPage((current) => Math.min(pages - 1, current + 1))}
                disabled={page + 1 >= pages || loading}
                aria-label={t('activity.nextPage')}
              >
                <ChevronRight size={16} />
              </button>
            </div>
          </footer>
        </div>
      )}
    </div>
  );
}

function HistoryRow({
  item,
  expanded,
  locale,
  onToggle,
}: {
  item: ActivityEntry;
  expanded: boolean;
  locale: string;
  onToggle: () => void;
}) {
  const { t } = useTranslation();
  return (
    <>
      <tr
        className="cursor-pointer border-b border-primary-800/50 hover:bg-primary-800/20"
        onClick={onToggle}
      >
        <td className="px-3 py-3 text-primary-500">
          {expanded ? <ChevronUp size={15} /> : <ChevronDown size={15} />}
        </td>
        <td className="max-w-64 truncate px-3 py-3 text-primary-200" title={item.filePath}>
          {item.fileName}
        </td>
        <td className="px-3 py-3 text-primary-400">{item.ruleName ?? '—'}</td>
        <td className="px-3 py-3 text-primary-400">{t(`activity.type.${item.type}`)}</td>
        <td className="px-3 py-3 text-xs text-primary-500">
          {new Intl.DateTimeFormat(locale, { dateStyle: 'short', timeStyle: 'medium' }).format(
            new Date(item.at),
          )}
        </td>
        <td className="px-3 py-3 text-right">
          <span className={item.success ? 'badge-success' : 'badge-danger'}>
            {t(item.success ? 'history.succeeded' : 'history.failed')}
          </span>
        </td>
      </tr>
      {expanded && (
        <tr className="border-b border-primary-800 bg-primary-950/40">
          <td colSpan={6} className="px-5 py-4">
            <ActivityDetails item={item} />
          </td>
        </tr>
      )}
    </>
  );
}

function ActivityDetails({ item }: { item: ActivityEntry }) {
  const { t } = useTranslation();
  const [retryState, setRetryState] = useState<'idle' | 'saving' | 'allowed'>('idle');
  const [retryError, setRetryError] = useState<string | null>(null);
  const needsCommandReview =
    item.error?.split(':', 1)[0]?.trim() === 'COMMAND_INTERRUPTED_REVIEW_REQUIRED' &&
    Boolean(item.ruleId && item.command);

  const allowCommandRetry = async () => {
    if (!item.ruleId || !item.command || !window.confirm(t('history.allowCommandRetryConfirm'))) {
      return;
    }
    setRetryState('saving');
    setRetryError(null);
    const response = await ipc.rules.allowInterruptedRetry(
      item.ruleId,
      item.filePath,
      item.command,
    );
    if (response.success) {
      setRetryState('allowed');
    } else {
      setRetryState('idle');
      setRetryError(readableIpcError(response.error, t));
    }
  };

  return (
    <dl className="grid gap-3 text-xs sm:grid-cols-2">
      <Detail label={t('history.sourcePath')} value={item.filePath} mono />
      {item.destination && (
        <Detail label={t('history.destinationPath')} value={item.destination} mono />
      )}
      {item.command && <Detail label={t('history.command')} value={item.command} mono />}
      {item.exitCode !== undefined && item.exitCode !== null && (
        <Detail label={t('activity.exitCode')} value={String(item.exitCode)} />
      )}
      {item.durationMs !== undefined && item.durationMs !== null && (
        <Detail label={t('activity.duration')} value={`${item.durationMs} ms`} />
      )}
      {item.timedOut && <Detail label={t('history.commandTimedOut')} value={t('common.yes')} />}
      {item.error && (
        <Detail label={t('history.error')} value={readableIpcError(item.error, t)} danger />
      )}
      {item.type === 'trash' && <Detail label={t('history.note')} value={t('history.trashHint')} />}
      {item.type === 'delete' && (
        <Detail label={t('history.note')} value={t('history.deletedNoUndo')} danger />
      )}
      {needsCommandReview && (
        <div className="rounded-md border border-amber-500/30 bg-amber-500/10 p-3 sm:col-span-2">
          {retryState === 'allowed' ? (
            <p className="text-amber-200">{t('history.commandRetryAllowed')}</p>
          ) : (
            <button
              className="btn-secondary"
              type="button"
              disabled={retryState === 'saving'}
              onClick={() => void allowCommandRetry()}
            >
              {retryState === 'saving' ? t('common.loading') : t('history.allowCommandRetry')}
            </button>
          )}
          {retryError && <p className="mt-2 text-red-300">{retryError}</p>}
        </div>
      )}
      {item.type === 'runCommand' && (
        <div className="sm:col-span-2">
          <dt className="mb-1 font-medium text-primary-500">{t('history.commandOutput')}</dt>
          <dd className="max-h-64 overflow-auto whitespace-pre-wrap break-all rounded-md border border-primary-800 bg-black/30 p-3 font-mono text-primary-300">
            {item.output || t('history.noOutput')}
          </dd>
        </div>
      )}
    </dl>
  );
}

function Detail({
  label,
  value,
  mono = false,
  danger = false,
}: {
  label: string;
  value: string;
  mono?: boolean;
  danger?: boolean;
}) {
  return (
    <div>
      <dt className="mb-1 font-medium text-primary-500">{label}</dt>
      <dd
        className={`break-all ${mono ? 'font-mono' : ''} ${danger ? 'text-red-300' : 'text-primary-300'}`}
      >
        {value}
      </dd>
    </div>
  );
}
