import { describe, it, expect } from 'vitest';

// Smoke test — verifies the test harness works. Real unit tests will live
// alongside `src-tauri` (Rust) and `src` (TS) once modules land.

describe('vitest setup', () => {
  it('runs basic assertions', () => {
    expect(1 + 1).toBe(2);
  });
});
