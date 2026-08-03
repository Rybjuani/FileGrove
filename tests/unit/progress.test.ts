import { describe, expect, it } from 'vitest';
import { progressPercent } from '../../src/lib/progress';

describe('organization progress', () => {
  it('turns processed files into a bounded percentage', () => {
    expect(progressPercent(50, 200)).toBe(25);
    expect(progressPercent(1, 3)).toBe(33);
    expect(progressPercent(220, 200)).toBe(100);
    expect(progressPercent(-1, 200)).toBe(0);
    expect(progressPercent(1, 0)).toBe(0);
  });
});
