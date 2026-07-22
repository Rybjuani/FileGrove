import { useEffect } from 'react';

/**
 * Apply the `theme` setting to <html> by toggling the `dark` class.
 * - `dark` → always dark
 * - `light` → always light
 * - `system` → follow `prefers-color-scheme`
 */
export function useTheme(theme?: 'dark' | 'light' | 'system') {
  useEffect(() => {
    const root = document.documentElement;
    const apply = (mode: 'dark' | 'light') => {
      root.classList.toggle('dark', mode === 'dark');
      root.classList.toggle('light', mode === 'light');
    };

    if (!theme || theme === 'system') {
      const mq = window.matchMedia('(prefers-color-scheme: dark)');
      apply(mq.matches ? 'dark' : 'light');
      const handler = (e: MediaQueryListEvent) => apply(e.matches ? 'dark' : 'light');
      mq.addEventListener('change', handler);
      return () => mq.removeEventListener('change', handler);
    }
    apply(theme);
  }, [theme]);
}
