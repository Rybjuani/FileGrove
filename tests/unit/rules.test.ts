import { describe, expect, it } from 'vitest';
import type { Rule } from '../../src/ipc/types';
import {
  isAbsoluteDestination,
  isManagedDestination,
  isValidCondition,
  isValidRule,
  parseConditionValue,
  suggestManagedDestination,
} from '../../src/lib/rules';

describe('rule form helpers', () => {
  it('normalizes comma-separated values without duplicates', () => {
    expect(parseConditionValue('extension', ' .jpg, .png, .jpg ')).toEqual(['.jpg', '.png']);
  });

  it('rejects invalid numeric and regular-expression conditions', () => {
    expect(isValidCondition({ type: 'olderThan', value: ['-1'] })).toBe(false);
    expect(isValidCondition({ type: 'regex', value: ['['] })).toBe(false);
  });

  it('accepts Linux absolute destinations and keeps Windows compatibility', () => {
    expect(isAbsoluteDestination('/home/user/Documents/{year}')).toBe(true);
    expect(isAbsoluteDestination('C:\\Users\\user\\Documents')).toBe(true);
    expect(isAbsoluteDestination('../Documents')).toBe(false);
  });

  it('accepts managed folders but rejects traversal and dynamic roots', () => {
    expect(isManagedDestination('Images')).toBe(true);
    expect(isManagedDestination('Images/{year}/{month}')).toBe(true);
    expect(isManagedDestination('../Images')).toBe(false);
    expect(isManagedDestination('{year}/Images')).toBe(false);
  });

  it('suggests one safe managed folder from a friendly rule name', () => {
    expect(suggestManagedDestination('  Fotos 2026  ')).toBe('Fotos 2026');
    expect(suggestManagedDestination('Fotos/Verano {year}')).toBe('Fotos Verano');
    expect(suggestManagedDestination('..')).toBe('');
  });

  it('validates a complete move rule', () => {
    const rule: Rule = {
      id: crypto.randomUUID(),
      name: 'Images',
      isQuickRule: false,
      enabled: true,
      order: 0,
      icon: 'Image',
      conditions: [{ type: 'extension', value: ['.jpg'] }],
      action: { type: 'move', destination: '/tmp/sorted' },
      createdAt: new Date().toISOString(),
    };
    expect(isValidRule(rule)).toBe(true);
    expect(
      isValidRule({
        ...rule,
        action: { type: 'move', destination: 'Images/{year}' },
      }),
    ).toBe(true);
  });
});
