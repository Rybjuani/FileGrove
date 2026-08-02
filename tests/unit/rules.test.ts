import { describe, expect, it } from 'vitest';
import type { Rule } from '../../src/ipc/types';
import {
  isAbsoluteDestination,
  isValidCondition,
  isValidRule,
  parseConditionValue,
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
  });
});
