import { useTranslation } from 'react-i18next';
import { useAppStore } from '../stores/useAppStore';
import type { Settings as SettingsT } from '../ipc/types';

export function Settings() {
  const { t, i18n } = useTranslation();
  const { settings, updateSettings } = useAppStore();
  const s = settings as SettingsT | null;

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-semibold text-primary-50">{t('settings.title')}</h1>
        <p className="mt-1 text-sm text-primary-400">{t('settings.subtitle')}</p>
      </div>

      {/* Appearance */}
      <section className="card space-y-4">
        <h2 className="text-sm font-semibold uppercase text-primary-300">
          {t('settings.appearance')}
        </h2>
        <Field label={t('settings.theme')}>
          <select
            className="input"
            value={s?.theme ?? 'dark'}
            onChange={(e) =>
              void updateSettings({
                theme: e.target.value as NonNullable<SettingsT['theme']>,
              })
            }
          >
            <option value="dark">{t('settings.dark')}</option>
            <option value="light">{t('settings.light')}</option>
            <option value="system">{t('settings.system')}</option>
          </select>
        </Field>
        <Field label={t('settings.language')} desc={t('settings.languageDesc')}>
          <select
            className="input"
            value={s?.language ?? 'en'}
            onChange={(e) => {
              void updateSettings({ language: e.target.value });
              void i18n.changeLanguage(e.target.value);
            }}
          >
            <option value="en">English</option>
            <option value="es">Español</option>
          </select>
        </Field>
      </section>

      {/* Behavior */}
      <section className="card space-y-4">
        <h2 className="text-sm font-semibold uppercase text-primary-300">
          {t('settings.behavior')}
        </h2>
        <Toggle
          label={t('settings.minimizeToTray')}
          desc={t('settings.minimizeToTrayDesc')}
          value={s?.minimizeToTray ?? true}
          onChange={(v) => void updateSettings({ minimizeToTray: v })}
        />
        <Toggle
          label={t('settings.showTrayIcon')}
          desc={t('settings.showTrayIconDesc')}
          value={s?.showTrayIcon ?? true}
          onChange={(v) => void updateSettings({ showTrayIcon: v })}
        />
        <Toggle
          label={t('settings.notifications')}
          desc={t('settings.notificationsDesc')}
          value={s?.notifications ?? true}
          onChange={(v) => void updateSettings({ notifications: v })}
        />
        <Field label={t('settings.conflictBehavior')} desc={t('settings.conflictBehaviorDesc')}>
          <select
            className="input"
            value={s?.conflictBehavior ?? 'rename'}
            onChange={(e) =>
              void updateSettings({
                conflictBehavior: e.target.value as NonNullable<SettingsT['conflictBehavior']>,
              })
            }
          >
            <option value="rename">{t('settings.rename')}</option>
            <option value="overwrite">{t('settings.overwrite')}</option>
            <option value="skip">{t('settings.skip')}</option>
          </select>
        </Field>
      </section>

      {/* Startup */}
      <section className="card space-y-4">
        <h2 className="text-sm font-semibold uppercase text-primary-300">
          {t('settings.startup')}
        </h2>
        <Toggle
          label={t('settings.launchAtStartup')}
          desc={t('settings.launchAtStartupDesc')}
          value={s?.launchAtStartup ?? false}
          onChange={(v) => void updateSettings({ launchAtStartup: v })}
        />
      </section>
    </div>
  );
}

function Field({
  label,
  desc,
  children,
}: {
  label: string;
  desc?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex items-start justify-between gap-4">
      <div className="min-w-0 flex-1">
        <p className="text-sm text-primary-200">{label}</p>
        {desc && <p className="mt-1 text-xs text-primary-500">{desc}</p>}
      </div>
      <div className="w-48 shrink-0">{children}</div>
    </div>
  );
}

function Toggle({
  label,
  desc,
  value,
  onChange,
}: {
  label: string;
  desc?: string;
  value: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <div className="flex items-start justify-between gap-4">
      <div className="min-w-0 flex-1">
        <p className="text-sm text-primary-200">{label}</p>
        {desc && <p className="mt-1 text-xs text-primary-500">{desc}</p>}
      </div>
      <button
        onClick={() => onChange(!value)}
        className={`relative h-6 w-11 shrink-0 rounded-full transition-colors ${
          value ? 'bg-accent-600' : 'bg-primary-700'
        }`}
      >
        <span
          className={`absolute top-0.5 left-0.5 inline-block h-5 w-5 transform rounded-full bg-white transition-transform ${
            value ? 'translate-x-5' : ''
          }`}
        />
      </button>
    </div>
  );
}
