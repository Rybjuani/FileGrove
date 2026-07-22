import type { Page } from '../App';
import { useTranslation } from 'react-i18next';
import { LayoutGrid, ListTree, History, Settings, Info, Trees } from 'lucide-react';

interface Props {
  currentPage: Page;
  onNavigate: (page: Page) => void;
  children: React.ReactNode;
}

export function MainLayout({ currentPage, onNavigate, children }: Props) {
  const { t } = useTranslation();

  const navItems: Array<{ id: Page; label: string; icon: React.ReactNode }> = [
    { id: 'dashboard', label: t('sidebar.dashboard'), icon: <LayoutGrid size={18} /> },
    { id: 'rules', label: t('sidebar.rules'), icon: <ListTree size={18} /> },
    { id: 'history', label: t('sidebar.history'), icon: <History size={18} /> },
    { id: 'settings', label: t('sidebar.settings'), icon: <Settings size={18} /> },
    { id: 'about', label: t('sidebar.about'), icon: <Info size={18} /> },
  ];

  return (
    <div className="flex h-screen overflow-hidden bg-primary-950 text-primary-50">
      {/* Sidebar */}
      <aside className="flex w-56 flex-col border-r border-primary-800 bg-primary-900">
        <div className="flex items-center gap-2 px-5 py-5">
          <Trees size={22} className="text-accent-500" />
          <span className="text-lg font-semibold">FileGrove</span>
        </div>
        <nav className="flex-1 space-y-1 px-3">
          {navItems.map((item) => (
            <button
              key={item.id}
              onClick={() => onNavigate(item.id)}
              className={`flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm transition-colors ${
                currentPage === item.id
                  ? 'bg-accent-600/20 text-accent-300'
                  : 'text-primary-300 hover:bg-primary-800 hover:text-primary-100'
              }`}
            >
              {item.icon}
              <span>{item.label}</span>
            </button>
          ))}
        </nav>
        <div className="px-5 py-3 text-xs text-primary-500">v0.1.0</div>
      </aside>

      {/* Main content */}
      <main className="flex-1 overflow-y-auto">
        <div className="mx-auto max-w-6xl px-8 py-8">{children}</div>
      </main>
    </div>
  );
}
