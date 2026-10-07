import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, cleanup, within, fireEvent } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import AuthModal from './AuthModal';
import * as useAuthModule from './useAuth';
import { PASSWORD_MIN_LEN } from '../auth/passwordRules';
import { authState } from '../test/helpers/mockAuth';

vi.mock('./useAuth');

const mockUseAuth = vi.mocked(useAuthModule.useAuth);

const loginSpy = vi.fn();
const registerSpy = vi.fn();
const onClose = vi.fn();

const VALID_PASSWORD = 'correct-horse-battery';

beforeEach(() => {
  loginSpy.mockResolvedValue(undefined);
  registerSpy.mockResolvedValue(undefined);
  mockUseAuth.mockReturnValue(authState({ login: loginSpy, register: registerSpy }));
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

/** Switch the modal from its default login mode into register mode. */
async function goToRegister(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole('button', { name: 'Register' }));
}

const passwordInput = () => screen.getByLabelText('Password');
const confirmInput = () => screen.getByLabelText('Confirm password');
const submitButton = (name: 'Log In' | 'Register') => screen.getByRole('button', { name });

describe('AuthModal — login mode', () => {
  it('renders neither the confirm field nor the checklist', () => {
    render(<AuthModal onClose={onClose} />);
    expect(screen.queryByLabelText('Confirm password')).not.toBeInTheDocument();
    expect(screen.queryByText(`At least ${PASSWORD_MIN_LEN} characters`)).not.toBeInTheDocument();
  });

  it('does not impose the register minLength, so legacy short passwords still submit', async () => {
    const user = userEvent.setup();
    render(<AuthModal onClose={onClose} />);

    // A pre-12-char-minimum account must still be able to sign in.
    expect(passwordInput()).not.toHaveAttribute('minLength');

    await user.type(screen.getByLabelText('Username or Email'), 'olduser');
    await user.type(passwordInput(), 'short1');
    await user.click(submitButton('Log In'));

    expect(loginSpy).toHaveBeenCalledWith({
      username_or_email: 'olduser',
      password: 'short1',
    });
    expect(onClose).toHaveBeenCalled();
  });
});

describe('AuthModal — register mode', () => {
  it('shows the confirm field and sets minLength to 12 on both password inputs', async () => {
    const user = userEvent.setup();
    render(<AuthModal onClose={onClose} />);
    await goToRegister(user);

    expect(confirmInput()).toBeInTheDocument();
    expect(passwordInput()).toHaveAttribute('minLength', String(PASSWORD_MIN_LEN));
    expect(confirmInput()).toHaveAttribute('minLength', String(PASSWORD_MIN_LEN));
  });

  it('ticks checklist rules live as the user types', async () => {
    const user = userEvent.setup();
    render(<AuthModal onClose={onClose} />);
    await goToRegister(user);

    const rules = () => screen.getByRole('list');
    const lengthRule = () =>
      within(rules()).getByText(`At least ${PASSWORD_MIN_LEN} characters`).closest('li')!;
    const nonDigitRule = () =>
      within(rules()).getByText('Contains at least one non-digit character').closest('li')!;

    // Empty: both unsatisfied.
    expect(within(lengthRule()).getByText('(not satisfied)')).toBeInTheDocument();
    expect(within(nonDigitRule()).getByText('(not satisfied)')).toBeInTheDocument();

    // All digits and too short: length fails, non-digit fails.
    await user.type(passwordInput(), '1234');
    expect(within(lengthRule()).getByText('(not satisfied)')).toBeInTheDocument();
    expect(within(nonDigitRule()).getByText('(not satisfied)')).toBeInTheDocument();

    // Long enough but still all digits: length passes, non-digit still fails.
    await user.type(passwordInput(), '12345678901');
    expect(within(lengthRule()).getByText('(satisfied)')).toBeInTheDocument();
    expect(within(nonDigitRule()).getByText('(not satisfied)')).toBeInTheDocument();

    // Add a letter: both pass.
    await user.type(passwordInput(), 'a');
    expect(within(lengthRule()).getByText('(satisfied)')).toBeInTheDocument();
    expect(within(nonDigitRule()).getByText('(satisfied)')).toBeInTheDocument();
  });

  it('blocks submit and shows an inline error while the confirm field mismatches', async () => {
    const user = userEvent.setup();
    render(<AuthModal onClose={onClose} />);
    await goToRegister(user);

    await user.type(screen.getByLabelText('Username'), 'newuser');
    await user.type(passwordInput(), VALID_PASSWORD);
    await user.type(confirmInput(), 'correct-horse-batteryX');

    expect(screen.getByText('Passwords do not match')).toBeInTheDocument();
    expect(submitButton('Register')).toBeDisabled();

    await user.click(submitButton('Register'));
    expect(registerSpy).not.toHaveBeenCalled();
  });

  it('enables submit once the passwords match and the rules pass', async () => {
    const user = userEvent.setup();
    render(<AuthModal onClose={onClose} />);
    await goToRegister(user);

    await user.type(screen.getByLabelText('Username'), 'newuser');
    await user.type(passwordInput(), VALID_PASSWORD);
    await user.type(confirmInput(), VALID_PASSWORD);

    expect(screen.queryByText('Passwords do not match')).not.toBeInTheDocument();
    expect(submitButton('Register')).toBeEnabled();

    await user.click(submitButton('Register'));
    expect(registerSpy).toHaveBeenCalledWith({
      username: 'newuser',
      password: VALID_PASSWORD,
    });
  });

  it('keeps submit disabled when the passwords match but fail the rules', async () => {
    const user = userEvent.setup();
    render(<AuthModal onClose={onClose} />);
    await goToRegister(user);

    await user.type(screen.getByLabelText('Username'), 'newuser');
    await user.type(passwordInput(), '123456789012'); // long enough, all digits
    await user.type(confirmInput(), '123456789012');

    expect(submitButton('Register')).toBeDisabled();
  });

  it('omits an empty email rather than sending a blank string', async () => {
    const user = userEvent.setup();
    render(<AuthModal onClose={onClose} />);
    await goToRegister(user);

    await user.type(screen.getByLabelText('Username'), 'newuser');
    await user.type(passwordInput(), VALID_PASSWORD);
    await user.type(confirmInput(), VALID_PASSWORD);
    await user.click(submitButton('Register'));

    expect(registerSpy).toHaveBeenCalledWith(
      expect.not.objectContaining({ email: expect.anything() }),
    );
  });

  it('clears the confirm field when switching back to login', async () => {
    const user = userEvent.setup();
    render(<AuthModal onClose={onClose} />);
    await goToRegister(user);

    await user.type(confirmInput(), VALID_PASSWORD);
    await user.click(screen.getByRole('button', { name: 'Log in' }));
    await goToRegister(user);

    expect(confirmInput()).toHaveValue('');
  });

  it('does not fold the rule text into the password input accessible name', async () => {
    const user = userEvent.setup();
    render(<AuthModal onClose={onClose} />);
    await goToRegister(user);

    // Regression guard: the checklist used to live inside the <label>, which
    // made screen readers announce every rule as part of the field name.
    expect(passwordInput()).toHaveAccessibleName('Password');
  });
});

describe('AuthModal — dialog semantics', () => {
  it('is a modal dialog named by its heading, in both modes', async () => {
    const user = userEvent.setup();
    render(<AuthModal onClose={onClose} />);
    const dialog = screen.getByRole('dialog', { name: 'Log In' });
    // A native <dialog> carries the role itself; the attributes would be noise.
    expect(dialog.tagName).toBe('DIALOG');
    expect(dialog).not.toHaveAttribute('role');
    expect(dialog).not.toHaveAttribute('aria-modal');

    await goToRegister(user);
    expect(screen.getByRole('dialog', { name: 'Create Account' })).toBeInTheDocument();
  });

  it('names the email field by its label and describes it with the helper text', async () => {
    const user = userEvent.setup();
    render(<AuthModal onClose={onClose} />);
    await goToRegister(user);
    const email = screen.getByRole('textbox', { name: 'Email (optional)' });
    expect(email).toHaveAccessibleDescription(/not used yet/i);
  });

  it('closes from the close button', async () => {
    const user = userEvent.setup();
    render(<AuthModal onClose={onClose} />);
    await user.click(screen.getByRole('button', { name: 'Close' }));
    expect(onClose).toHaveBeenCalledOnce();
  });
});

// The polyfill in test/setup.ts only toggles `open`; these cover our wiring
// to the dialog API. Focus trapping, real Escape handling and focus
// restoration are browser behaviour and are verified in Chromium instead.
describe('AuthModal — native dialog wiring', () => {
  it('opens itself as a modal on mount and focuses the first field', () => {
    const showModal = vi.spyOn(HTMLDialogElement.prototype, 'showModal');
    render(<AuthModal onClose={onClose} />);
    expect(showModal).toHaveBeenCalledOnce();
    expect(screen.getByRole('dialog')).toHaveAttribute('open');
    expect(screen.getByLabelText('Username or Email')).toHaveFocus();
    showModal.mockRestore();
  });

  it('closes the native dialog when it unmounts, so the browser restores focus', () => {
    const close = vi.spyOn(HTMLDialogElement.prototype, 'close');
    const { unmount } = render(<AuthModal onClose={onClose} />);
    const dialog = screen.getByRole('dialog');
    unmount();
    expect(close).toHaveBeenCalledOnce();
    expect(close.mock.contexts[0]).toBe(dialog);
    close.mockRestore();
  });

  it.each(['login', 'register'] as const)(
    'routes Escape (the cancel event) to onClose in %s mode and leaves closing to the parent',
    async (mode) => {
      const user = userEvent.setup();
      render(<AuthModal onClose={onClose} />);
      if (mode === 'register') await goToRegister(user);

      const dialog = screen.getByRole('dialog');
      const cancel = new Event('cancel', { cancelable: true });
      fireEvent(dialog, cancel);

      expect(onClose).toHaveBeenCalledOnce();
      expect(cancel.defaultPrevented).toBe(true);
      expect(dialog).toHaveAttribute('open');
    },
  );

  it('reports a close the browser forced on its own', () => {
    render(<AuthModal onClose={onClose} />);
    // e.g. a repeated Escape that Chrome won't let the page cancel.
    (screen.getByRole('dialog') as HTMLDialogElement).close();
    expect(onClose).toHaveBeenCalledOnce();
  });

  it('ignores a close event that arrives after the dialog was reopened', () => {
    render(<AuthModal onClose={onClose} />);
    // StrictMode's dev effect re-run: cleanup close() queues `close`, the
    // effect reopens, and the stale event lands on an open dialog.
    fireEvent(screen.getByRole('dialog'), new Event('close'));
    expect(onClose).not.toHaveBeenCalled();
  });
});
