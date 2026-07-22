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

export type Page = 'dashboard' | 'rules' | 'history' | 'settings' | 'about';

export default function App() {
  const [page, setPage] = useState<Page>('dashboard');
  const { settings, loadSettings } = useAppStore();
  useTheme(settings?.theme);

  useEffect(() => {
    loadSettings();
  }, [loadSettings]);

  useEffect(() => {
    if (settings?.language) {
      void i18n.changeLanguage(settings.language);
    }
  }, [settings?.language]);

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
      {renderPage()}
    </MainLayout>
  );
}
