import { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Trash2 } from 'lucide-react';
import { ipc } from '../ipc';
import type { ActivityEntry } from '../ipc/types';

interface Props {
  onNavigateToRules: () => void;
}

export function History({ onNavigateToRules: _ }: Props) {
  const { t } = useTranslation();
  const [items, setItems] = useState<ActivityEntry[]>([]);
  const [total, setTotal] = useState(0);
  const [filter, setFilter] = useState<'all' | 'success' | 'failure'>('all');

  const load = useCallback(async () => {
    const opts: Record<string, unknown> = { limit: 50, offset: 0 };
    if (filter === 'success') opts.success = true;
    if (filter === 'failure') opts.success = false;
    const res = await ipc.activity.list(opts);
    if (res.success && res.data) {
      setItems(res.data.items);
      setTotal(res.data.total);
    }
  }, [filter]);

  useEffect(() => {
    void load();
  }, [load]);

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold text-primary-50">{t('history.title')}</h1>
          <p className="mt-1 text-sm text-primary-400">{t('history.subtitle')}</p>
        </div>
        <div className="flex items-center gap-2">
          <div className="grid grid-cols-3 gap-1 rounded-md border border-primary-800 bg-primary-950 p-1 text-xs">
            {(['all', 'success', 'failure'] as const).map((f) => (
              <button
                key={f}
                onClick={() => setFilter(f)}
                className={`rounded px-3 py-1 transition-colors ${
                  filter === f
                    ? 'bg-accent-600 text-white'
                    : 'text-primary-400 hover:bg-primary-800'
                }`}
              >
                {t(`history.filter${f[0].toUpperCase()}${f.slice(1)}`)}
              </button>
            ))}
          </div>
          <button
            className="btn-ghost text-red-400"
            onClick={async () => {
              if (confirm(t('history.clearConfirm'))) {
                await ipc.activity.clear();
                void load();
              }
            }}
          >
            <Trash2 size={14} />
            {t('history.clearAll')}
          </button>
        </div>
      </div>

      {items.length === 0 ? (
        <div className="card py-12 text-center">
          <p className="text-sm text-primary-400">{t('history.empty')}</p>
          <p className="mt-2 text-xs text-primary-500">{t('history.emptyDesc')}</p>
        </div>
      ) : (
        <div className="card overflow-hidden p-0">
          <table className="w-full text-left text-sm">
            <thead className="border-b border-primary-800 bg-primary-950/50 text-xs uppercase text-primary-500">
              <tr>
                <th className="px-4 py-3">{t('activity.file')}</th>
                <th className="px-4 py-3">{t('activity.rule')}</th>
                <th className="px-4 py-3">Type</th>
                <th className="px-4 py-3">When</th>
                <th className="px-4 py-3 text-right">Status</th>
              </tr>
            </thead>
            <tbody>
              {items.map((item) => (
                <tr key={item.id} className="border-b border-primary-800/50 last:border-0">
                  <td className="px-4 py-3 text-primary-200" title={item.filePath}>
                    {item.fileName}
                  </td>
                  <td className="px-4 py-3 text-primary-400">{item.ruleName ?? '—'}</td>
                  <td className="px-4 py-3 text-primary-400">{item.type}</td>
                  <td className="px-4 py-3 text-xs text-primary-500">
                    {new Date(item.at).toLocaleString()}
                  </td>
                  <td className="px-4 py-3 text-right">
                    {item.success ? (
                      <span className="badge-success">OK</span>
                    ) : (
                      <span className="badge-danger" title={item.error ?? ''}>
                        FAIL
                      </span>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          <p className="px-4 py-2 text-xs text-primary-500">
            {items.length} / {total}
          </p>
        </div>
      )}
    </div>
  );
}
