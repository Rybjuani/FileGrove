import { FolderPlus, Play, ShieldCheck } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useDialogFocus } from '../lib/dialog';

interface Props {
  saving: boolean;
  onComplete: () => void;
}

const STEPS = [
  { key: 'folder', icon: FolderPlus },
  { key: 'preview', icon: ShieldCheck },
  { key: 'automatic', icon: Play },
] as const;

export function OnboardingDialog({ saving, onComplete }: Props) {
  const { t } = useTranslation();
  const dialogRef = useDialogFocus<HTMLElement>();

  return (
    <div className="fixed inset-0 z-[60] flex items-center justify-center bg-black/75 p-4">
      <section
        ref={dialogRef}
        className="card max-h-[92vh] w-full max-w-2xl overflow-y-auto p-6 sm:p-8"
        role="dialog"
        aria-modal="true"
        aria-busy={saving}
        aria-labelledby="onboarding-title"
        aria-describedby="onboarding-intro"
        tabIndex={-1}
      >
        <div className="mx-auto flex h-12 w-12 items-center justify-center rounded-full bg-accent-500/15 text-accent-300">
          <ShieldCheck size={25} aria-hidden="true" />
        </div>
        <h1
          id="onboarding-title"
          className="mt-4 text-center text-2xl font-semibold text-primary-50"
        >
          {t('onboarding.title')}
        </h1>
        <p
          id="onboarding-intro"
          className="mx-auto mt-2 max-w-xl text-center text-sm text-primary-400"
        >
          {t('onboarding.intro')}
        </p>

        <ol className="mt-7 grid gap-3 sm:grid-cols-3">
          {STEPS.map(({ key, icon: Icon }, index) => (
            <li key={key} className="rounded-lg border border-primary-800 bg-primary-950/50 p-4">
              <div className="flex items-center justify-between text-accent-300">
                <Icon size={20} aria-hidden="true" />
                <span className="text-xs font-semibold" aria-hidden="true">
                  {index + 1}
                </span>
              </div>
              <h2 className="mt-3 text-sm font-semibold text-primary-100">
                {t(`onboarding.steps.${key}.title`)}
              </h2>
              <p className="mt-1 text-xs leading-relaxed text-primary-400">
                {t(`onboarding.steps.${key}.body`)}
              </p>
            </li>
          ))}
        </ol>

        <p className="mt-4 rounded-md bg-accent-500/10 p-3 text-xs leading-relaxed text-accent-200">
          {t('onboarding.undoNote')}
        </p>

        <div className="mt-6 flex justify-center">
          <button
            type="button"
            className="btn-primary min-w-48"
            disabled={saving}
            onClick={onComplete}
            autoFocus
          >
            {t(saving ? 'onboarding.saving' : 'onboarding.start')}
          </button>
        </div>
      </section>
    </div>
  );
}
