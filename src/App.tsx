import { useEffect, useState } from 'react';
import { MainLayout } from './components/MainLayout';
import { Dashboard } from './pages/Dashboard';
import { Rules } from './pages/Rules';
import { History } from './pages/History';
import { Settings } from './pages/Settings';
import { About } from './pages/About';
import { useAppStore } from './stores/useAppStore';
import { useTheme } from './lib/theme';
import i18n from './i18n';
import { useTranslation } from 'react-i18next';
import { X } from 'lucide-react';
import { readableIpcError } from './lib/errors';
import { ipc } from './ipc';

export type Page = 'dashboard' | 'rules' | 'history' | 'settings' | 'about';

export default function App() {
  const { t } = useTranslation();
  const [page, setPage] = useState<Page>('dashboard');
  const { settings, loadSettings, error, clearError, reportError } = useAppStore();
  useTheme(settings?.theme);

  useEffect(() => {
    loadSettings();
  }, [loadSettings]);

  useEffect(() => {
    if (settings?.language) {
      void i18n.changeLanguage(settings.language);
    }
  }, [settings?.language]);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void ipc.files
      .onBackgroundError(({ error }) => reportError(error))
      .then((stop) => {
        if (disposed) stop();
        else unlisten = stop;
      });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [reportError]);

  const renderPage = () => {
    switch (page) {
      case 'dashboard':
        return (
          <Dashboard
            onNavigateToRules={() => setPage('rules')}
            onNavigateToHistory={() => setPage('history')}
          />
        );
      case 'rules':
        return <Rules />;
      case 'history':
        return <History onNavigateToRules={() => setPage('rules')} />;
      case 'settings':
        return <Settings />;
      case 'about':
        return <About />;
      default:
        return (
          <Dashboard
            onNavigateToRules={() => setPage('rules')}
            onNavigateToHistory={() => setPage('history')}
          />
        );
    }
  };

  return (
    <MainLayout currentPage={page} onNavigate={setPage}>
      {error && (
        <div
          role="alert"
          className="mb-5 flex items-start justify-between gap-3 rounded-md border border-red-500/30 bg-red-500/10 p-3 text-sm text-red-300"
        >
          <span>{readableIpcError(error, t)}</span>
          <button
            className="shrink-0 rounded p-1 hover:bg-red-500/10"
            onClick={clearError}
            aria-label={t('common.dismiss')}
          >
            <X size={15} />
          </button>
        </div>
      )}
      {renderPage()}
    </MainLayout>
  );
}
