import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { useDialogFocus } from '../../src/lib/dialog';

function TestDialog({ onClose }: { onClose: () => void }) {
  const dialogRef = useDialogFocus<HTMLElement>(onClose);
  return (
    <section ref={dialogRef} role="dialog" tabIndex={-1}>
      <button type="button">First</button>
      <button type="button">Last</button>
    </section>
  );
}

describe('dialog keyboard focus', () => {
  it('traps Tab, handles Escape, and restores the opener', () => {
    const opener = document.createElement('button');
    document.body.append(opener);
    opener.focus();
    const onClose = vi.fn();
    const { unmount } = render(<TestDialog onClose={onClose} />);
    const dialog = screen.getByRole('dialog');
    const first = screen.getByRole('button', { name: 'First' });
    const last = screen.getByRole('button', { name: 'Last' });

    expect(first).toHaveFocus();
    last.focus();
    fireEvent.keyDown(dialog, { key: 'Tab' });
    expect(first).toHaveFocus();
    fireEvent.keyDown(dialog, { key: 'Tab', shiftKey: true });
    expect(last).toHaveFocus();
    fireEvent.keyDown(dialog, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledOnce();

    unmount();
    expect(opener).toHaveFocus();
    opener.remove();
  });
});
