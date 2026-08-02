import { describe, expect, it } from 'vitest';
import i18n from '../../src/i18n';
import { ipcErrorCode, readableIpcError } from '../../src/lib/errors';

describe('IPC error presentation', () => {
  it('maps stable backend codes without exposing implementation details', () => {
    const t = i18n.getFixedT('en');
    expect(readableIpcError('SCAN_LIMIT_REACHED: internal detail', t)).toBe(
      'The scan stopped at the 100,000-file safety limit. Add exclusions or reduce depth.',
    );
    expect(ipcErrorCode('scan_limit_reached: detail')).toBe('SCAN_LIMIT_REACHED');
  });

  it('preserves the useful command exit code', () => {
    const t = i18n.getFixedT('en');
    expect(readableIpcError('COMMAND_EXIT: 17', t)).toContain('17');
  });

  it('uses a safe fallback for non-string errors', () => {
    const t = i18n.getFixedT('en');
    expect(readableIpcError(new Error('secret'), t)).toBe('Something went wrong');
  });
});
