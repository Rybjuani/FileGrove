import { describe, expect, it } from 'vitest';
import en from '../../src/i18n/locales/en.json';
import es from '../../src/i18n/locales/es.json';

function leafKeys(value: unknown, prefix = ''): string[] {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return [prefix];
  return Object.entries(value as Record<string, unknown>).flatMap(([key, child]) =>
    leafKeys(child, prefix ? `${prefix}.${key}` : key),
  );
}

describe('translation catalogs', () => {
  it('keep English and Spanish structurally aligned', () => {
    expect(leafKeys(es).sort()).toEqual(leafKeys(en).sort());
  });

  it('contains Linux-first examples instead of Windows-only paths', () => {
    expect(en.rules.destinationPlaceholder).toContain('/home/');
    expect(es.rules.destinationPlaceholder).toContain('/home/');
  });
});
