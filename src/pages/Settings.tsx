import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useAppStore } from '../stores/useAppStore';
import type { Settings as SettingsT } from '../ipc/types';
import { readableIpcError } from '../lib/errors';

export function Settings() {
  const { t, i18n } = useTranslation();
  const { settings, updateSettings } = useAppStore();
  const [saving, setSaving] = useState<string | null>(null);
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(null);
  const s = settings as SettingsT | null;

  const save = async (key: string, partial: Partial<SettingsT>) => {
    setSaving(key);
    setMessage(null);
    const ok = await updateSettings(partial);
    setSaving(null);
    setMessage({
      ok,
      text: ok
        ? t('settings.saved')
        : readableIpcError(useAppStore.getState().error ?? undefined, t),
    });
    return ok;
  };

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-semibold text-primary-50">{t('settings.title')}</h1>
        <p className="mt-1 text-sm text-primary-400">{t('settings.subtitle')}</p>
      </div>

      {message && (
        <p
          role="status"
          className={`rounded-md border p-3 text-sm ${
            message.ok
              ? 'border-accent-500/30 bg-accent-500/10 text-accent-300'
              : 'border-red-500/30 bg-red-500/10 text-red-300'
          }`}
        >
          {message.text}
        </p>
      )}

      <section className="card space-y-4">
        <h2 className="text-sm font-semibold uppercase text-primary-300">
          {t('settings.appearance')}
        </h2>
        <Field label={t('settings.theme')}>
          <select
            className="input"
            value={s?.theme ?? 'dark'}
            disabled={saving === 'theme'}
            onChange={(event) =>
              void save('theme', {
                theme: event.target.value as NonNullable<SettingsT['theme']>,
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
            disabled={saving === 'language'}
            onChange={(event) => {
              const language = event.target.value;
              void save('language', { language }).then((ok) => {
                if (ok) void i18n.changeLanguage(language);
              });
            }}
          >
            <option value="en">English</option>
            <option value="es">Español</option>
          </select>
        </Field>
      </section>

      <section className="card space-y-4">
        <h2 className="text-sm font-semibold uppercase text-primary-300">
          {t('settings.behavior')}
        </h2>
        <Toggle
          label={t('settings.minimizeToTray')}
          desc={t('settings.minimizeToTrayDesc')}
          value={s?.minimizeToTray ?? true}
          disabled={saving === 'minimizeToTray'}
          onChange={(value) => void save('minimizeToTray', { minimizeToTray: value })}
        />
        <Toggle
          label={t('settings.showTrayIcon')}
          desc={t('settings.showTrayIconDesc')}
          value={s?.showTrayIcon ?? true}
          disabled={saving === 'showTrayIcon'}
          onChange={(value) => void save('showTrayIcon', { showTrayIcon: value })}
        />
        <Toggle
          label={t('settings.notifications')}
          desc={t('settings.notificationsDesc')}
          value={s?.notifications ?? true}
          disabled={saving === 'notifications'}
          onChange={(value) => void save('notifications', { notifications: value })}
        />
        <Field label={t('settings.conflictBehavior')} desc={t('settings.conflictBehaviorDesc')}>
          <select
            className="input"
            value={s?.conflictBehavior ?? 'rename'}
            disabled={saving === 'conflictBehavior'}
            onChange={(event) => {
              const conflictBehavior = event.target.value as NonNullable<
                SettingsT['conflictBehavior']
              >;
              if (
                conflictBehavior === 'overwrite' &&
                s?.conflictBehavior !== 'overwrite' &&
                !window.confirm(t('settings.overwriteConfirm'))
              ) {
                event.currentTarget.value = s?.conflictBehavior ?? 'rename';
                return;
              }
              void save('conflictBehavior', { conflictBehavior });
            }}
          >
            <option value="rename">{t('settings.rename')}</option>
            <option value="overwrite">{t('settings.overwrite')}</option>
            <option value="skip">{t('settings.skip')}</option>
          </select>
          {s?.conflictBehavior === 'overwrite' && (
            <p className="mt-2 text-xs text-yellow-400">{t('settings.overwriteWarning')}</p>
          )}
        </Field>
        <Field label={t('settings.duplicateBehavior')} desc={t('settings.duplicateBehaviorDesc')}>
          <select
            className="input"
            value={s?.duplicateBehavior ?? 'keepBoth'}
            disabled={saving === 'duplicateBehavior'}
            onChange={(event) => {
              const duplicateBehavior = event.target.value as NonNullable<
                SettingsT['duplicateBehavior']
              >;
              if (
                duplicateBehavior === 'delete' &&
                s?.duplicateBehavior !== 'delete' &&
                !window.confirm(t('settings.deleteDuplicateConfirm'))
              ) {
                event.currentTarget.value = s?.duplicateBehavior ?? 'keepBoth';
                return;
              }
              void save('duplicateBehavior', { duplicateBehavior });
            }}
          >
            <option value="keepBoth">{t('settings.keepBothDuplicates')}</option>
            <option value="trash">{t('settings.trashDuplicates')}</option>
            <option value="delete">{t('settings.deleteDuplicates')}</option>
          </select>
          {s?.duplicateBehavior === 'delete' && (
            <p className="mt-2 text-xs text-red-300">{t('settings.deleteDuplicateWarning')}</p>
          )}
        </Field>
      </section>

      <section className="card space-y-4">
        <h2 className="text-sm font-semibold uppercase text-primary-300">
          {t('settings.startup')}
        </h2>
        <Toggle
          label={t('settings.launchAtStartup')}
          desc={t('settings.launchAtStartupDesc')}
          value={s?.launchAtStartup ?? false}
          disabled={saving === 'launchAtStartup'}
          onChange={(value) => void save('launchAtStartup', { launchAtStartup: value })}
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
  disabled,
  onChange,
}: {
  label: string;
  desc?: string;
  value: boolean;
  disabled?: boolean;
  onChange: (value: boolean) => void;
}) {
  return (
    <div className="flex items-start justify-between gap-4">
      <div className="min-w-0 flex-1">
        <p className="text-sm text-primary-200">{label}</p>
        {desc && <p className="mt-1 text-xs text-primary-500">{desc}</p>}
      </div>
      <button
        type="button"
        role="switch"
        aria-checked={value}
        aria-label={label}
        disabled={disabled}
        onClick={() => onChange(!value)}
        className={`relative h-6 w-11 shrink-0 rounded-full transition-colors disabled:opacity-50 ${
          value ? 'bg-accent-600' : 'bg-primary-700'
        }`}
      >
        <span
          className={`absolute left-0.5 top-0.5 inline-block h-5 w-5 transform rounded-full bg-white transition-transform ${
            value ? 'translate-x-5' : ''
          }`}
        />
      </button>
    </div>
  );
}
