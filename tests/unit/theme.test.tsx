import { act, renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useTheme } from '../../src/lib/theme';

describe('useTheme', () => {
  beforeEach(() => {
    document.documentElement.className = '';
  });

  it('applies explicit light and dark themes', () => {
    const { rerender } = renderHook(({ theme }) => useTheme(theme), {
      initialProps: { theme: 'dark' as 'dark' | 'light' | 'system' },
    });
    expect(document.documentElement).toHaveClass('dark');
    rerender({ theme: 'light' });
    expect(document.documentElement).toHaveClass('light');
    expect(document.documentElement).not.toHaveClass('dark');
  });

  it('follows system theme changes and cleans up the listener', () => {
    let listener: ((event: MediaQueryListEvent) => void) | undefined;
    const removeEventListener = vi.fn();
    vi.stubGlobal(
      'matchMedia',
      vi.fn(() => ({
        matches: true,
        addEventListener: (_name: string, next: (event: MediaQueryListEvent) => void) => {
          listener = next;
        },
        removeEventListener,
      })),
    );
    const { unmount } = renderHook(() => useTheme('system'));
    expect(document.documentElement).toHaveClass('dark');
    act(() => listener?.({ matches: false } as MediaQueryListEvent));
    expect(document.documentElement).toHaveClass('light');
    unmount();
    expect(removeEventListener).toHaveBeenCalledOnce();
    vi.unstubAllGlobals();
  });
});
