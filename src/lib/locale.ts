export type UiLocale = 'es' | 'en';

export function detectUiLocale(language: string | undefined | null): UiLocale {
  return (language ?? '').trim().toLowerCase().startsWith('es') ? 'es' : 'en';
}

export interface FirstRunPatch {
  language?: UiLocale;
  theme?: 'dark' | 'light' | 'system';
}

export function firstRunLocalePatch(
  settings: { language?: string; theme?: string; onboardingCompleted?: boolean } | null,
  navLanguage: string | undefined | null,
): FirstRunPatch {
  if (!settings || settings.onboardingCompleted === true) return {};
  const locale = detectUiLocale(navLanguage);
  const patch: FirstRunPatch = {};
  if (settings.language !== locale) patch.language = locale;
  if (settings.theme !== 'system') patch.theme = 'system';
  return patch;
}
