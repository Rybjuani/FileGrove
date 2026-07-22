import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus, FolderPlus, Search } from 'lucide-react';
import { useAppStore } from '../stores/useAppStore';
import { FolderCard } from '../components/FolderCard';

interface Props {
  onNavigateToRules: () => void;
  onNavigateToHistory: () => void;
}

export function Dashboard({ onNavigateToRules, onNavigateToHistory: _ }: Props) {
  const { t } = useTranslation();
  const { folders, rules, addFolder, loadFolders, loadRules } = useAppStore();
  const [query, setQuery] = useState('');

  useEffect(() => {
    loadFolders();
    loadRules();
  }, [loadFolders, loadRules]);

  const filtered = folders.filter(
    (f) =>
      f.name.toLowerCase().includes(query.toLowerCase()) ||
      f.path.toLowerCase().includes(query.toLowerCase()),
  );

  const enabledRules = rules.filter((r) => r.enabled).length;

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold text-primary-50">{t('dashboard.title')}</h1>
          <p className="mt-1 text-sm text-primary-400">{t('dashboard.subtitle')}</p>
        </div>
        <button className="btn-primary" onClick={() => void addFolder()}>
          <Plus size={16} />
          {t('dashboard.addFolder')}
        </button>
      </div>

      {folders.length > 0 && (
        <div className="relative w-full max-w-md">
          <Search size={16} className="absolute left-3 top-1/2 -translate-y-1/2 text-primary-500" />
          <input
            className="input pl-9"
            placeholder={t('dashboard.searchPlaceholder')}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        </div>
      )}

      {folders.length === 0 ? (
        <div className="card flex flex-col items-center justify-center space-y-4 py-16 text-center">
          <FolderPlus size={48} className="text-primary-600" />
          <div>
            <h2 className="text-lg font-medium text-primary-200">{t('dashboard.noFolders')}</h2>
            <p className="mt-2 max-w-md text-sm text-primary-400">{t('dashboard.noFoldersDesc')}</p>
          </div>
          <button className="btn-primary" onClick={() => void addFolder()}>
            <Plus size={16} />
            {t('dashboard.addFirstFolder')}
          </button>
        </div>
      ) : (
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {filtered.map((folder) => (
            <FolderCard key={folder.id} folder={folder} onNavigateToRules={onNavigateToRules} />
          ))}
        </div>
      )}

      {rules.length > 0 && (
        <p className="text-xs text-primary-500">
          {t('dashboard.rulesActive', { enabled: enabledRules, total: rules.length })}
        </p>
      )}
    </div>
  );
}
