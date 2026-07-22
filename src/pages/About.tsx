import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Trees, Github, Globe, Heart } from 'lucide-react';
import { ipc } from '../ipc';

export function About() {
  const { t } = useTranslation();
  const [version, setVersion] = useState('0.1.0');

  useEffect(() => {
    void ipc.app.getVersion().then((res) => {
      if (res.success && res.data) setVersion(res.data);
    });
  }, []);

  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-semibold text-primary-50">{t('about.title')}</h1>

      <div className="card flex items-center gap-4">
        <div className="flex h-16 w-16 items-center justify-center rounded-xl bg-accent-600/20">
          <Trees size={32} className="text-accent-400" />
        </div>
        <div>
          <h2 className="text-xl font-semibold text-primary-50">FileGrove</h2>
          <p className="text-sm text-primary-400">{t('about.description')}</p>
          <p className="mt-1 text-xs text-primary-500">
            {t('about.version')}: <span className="text-primary-300">{version}</span>
          </p>
        </div>
      </div>

      <div className="card space-y-3 text-sm">
        <Row label={t('about.license')} value="Apache 2.0" />
        <Row label={t('about.openSource')} value={t('about.madeWith')} />
        <Row label={t('about.inspiredBy')} value="File Arbor by Furkan Bagci" />
      </div>

      <div className="card space-y-2">
        <h3 className="text-sm font-semibold uppercase text-primary-300">
          {t('about.acknowledgements')}
        </h3>
        <a
          href="https://github.com/filegrove/filegrove"
          target="_blank"
          rel="noopener noreferrer"
          className="flex items-center gap-2 text-sm text-accent-400 hover:text-accent-300"
        >
          <Github size={16} /> GitHub
        </a>
        <a
          href="https://filegrove.org"
          target="_blank"
          rel="noopener noreferrer"
          className="flex items-center gap-2 text-sm text-accent-400 hover:text-accent-300"
        >
          <Globe size={16} /> {t('about.website')}
        </a>
        <p className="flex items-center gap-2 text-xs text-primary-500">
          <Heart size={14} /> {t('about.openSource')}
        </p>
      </div>
    </div>
  );
}

function Row({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex justify-between">
      <span className="text-primary-400">{label}</span>
      <span className="text-primary-200">{value}</span>
    </div>
  );
}
