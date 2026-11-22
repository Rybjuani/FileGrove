import { describe, expect, it } from 'vitest';
import { detectUiLocale, firstRunLocalePatch } from '../../src/lib/locale';

describe('detectUiLocale', () => {
  it('detects Spanish variants', () => {
    expect(detectUiLocale('es')).toBe('es');
    expect(detectUiLocale('es-ES')).toBe('es');
    expect(detectUiLocale('es-AR')).toBe('es');
    expect(detectUiLocale('ES-MX')).toBe('es');
  });
  it('falls back to English for non-Spanish or missing', () => {
    expect(detectUiLocale('en-US')).toBe('en');
    expect(detectUiLocale('fr-FR')).toBe('en');
    expect(detectUiLocale(undefined)).toBe('en');
    expect(detectUiLocale(null)).toBe('en');
    expect(detectUiLocale('')).toBe('en');
  });
});

describe('firstRunLocalePatch', () => {
  it('seeds OS language and system theme on first run', () => {
    expect(firstRunLocalePatch({ language: 'en', theme: 'dark', onboardingCompleted: false }, 'es-AR'))
      .toEqual({ language: 'es', theme: 'system' });
  });
  it('does nothing after onboarding is completed', () => {
    expect(firstRunLocalePatch({ language: 'en', theme: 'dark', onboardingCompleted: true }, 'es-AR'))
      .toEqual({});
  });
  it('does nothing when already configured to the detected locale', () => {
    expect(firstRunLocalePatch({ language: 'es', theme: 'system', onboardingCompleted: false }, 'es-ES'))
      .toEqual({});
  });
  it('returns empty patch for null settings', () => {
    expect(firstRunLocalePatch(null, 'es')).toEqual({});
  });
});
