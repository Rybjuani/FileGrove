import { fireEvent, render, screen } from '@testing-library/react';
import { I18nextProvider } from 'react-i18next';
import { describe, expect, it, vi } from 'vitest';
import { OnboardingDialog } from '../../src/components/OnboardingDialog';
import i18n from '../../src/i18n';

describe('first-run onboarding', () => {
  it('explains the safe path into Automatic mode and completes explicitly', () => {
    const onComplete = vi.fn();
    render(
      <I18nextProvider i18n={i18n}>
        <OnboardingDialog saving={false} onComplete={onComplete} />
      </I18nextProvider>,
    );

    expect(screen.getByRole('dialog')).toHaveAttribute('aria-modal', 'true');
    expect(screen.getByText('Preview before changing anything')).toBeInTheDocument();
    expect(screen.getByText('Turn on Automatic when ready')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Start safely' }));
    expect(onComplete).toHaveBeenCalledOnce();
  });
});
