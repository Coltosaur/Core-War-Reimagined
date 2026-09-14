import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { MockedFunction } from 'vitest';
import { render, screen, cleanup, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import AuthModal from './AuthModal';
import * as AuthContextModule from './AuthContext';
import { PASSWORD_MIN_LEN } from '../auth/passwordRules';

vi.mock('./AuthContext');

type UseAuthReturn = ReturnType<typeof AuthContextModule.useAuth>;
const mockUseAuth = AuthContextModule.useAuth as MockedFunction<() => UseAuthReturn>;

const loginSpy = vi.fn();
const registerSpy = vi.fn();
const onClose = vi.fn();

const VALID_PASSWORD = 'correct-horse-battery';

beforeEach(() => {
  loginSpy.mockResolvedValue(undefined);
  registerSpy.mockResolvedValue(undefined);
  mockUseAuth.mockReturnValue({
    user: null,
    loading: false,
    login: loginSpy,
    register: registerSpy,
    logout: vi.fn(),
  });
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
