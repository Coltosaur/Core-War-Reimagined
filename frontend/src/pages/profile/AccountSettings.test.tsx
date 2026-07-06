import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { MockedFunction } from 'vitest';
import { render, screen, waitFor, cleanup } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import AccountSettings from './AccountSettings';
import { checkPasswordRules, passwordMeetsRules } from './passwordRules';
import * as accountApi from '../../api/account';
import * as AuthContextModule from '../../api/AuthContext';
import { ApiError } from '../../api/client';

vi.mock('../../api/account');
vi.mock('../../api/AuthContext');

type UseAuthReturn = ReturnType<typeof AuthContextModule.useAuth>;

const mockUseAuth = AuthContextModule.useAuth as MockedFunction<() => UseAuthReturn>;
const mockGetAccount = accountApi.getAccount as MockedFunction<typeof accountApi.getAccount>;
const mockChangePassword = accountApi.changePassword as MockedFunction<
  typeof accountApi.changePassword
>;

const logoutSpy = vi.fn();

beforeEach(() => {
  mockUseAuth.mockReturnValue({
    user: { user_id: 'u1', username: 'vale' },
    loading: false,
    login: vi.fn(),
    register: vi.fn(),
    logout: logoutSpy,
  });
  mockGetAccount.mockResolvedValue({ email: 'vale@example.com' });
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe('checkPasswordRules', () => {
  it('marks length as ok only at >= 12 chars', () => {
    const short = checkPasswordRules('short').find((r) => r.id === 'length')!;
    expect(short.ok).toBe(false);
    const at12 = checkPasswordRules('a'.repeat(12)).find((r) => r.id === 'length')!;
    expect(at12.ok).toBe(true);
  });

  it('marks non_digit as failing on pure-numeric passwords', () => {
    const digits = checkPasswordRules('1'.repeat(20)).find((r) => r.id === 'non_digit')!;
    expect(digits.ok).toBe(false);
    const mixed = checkPasswordRules('abcdefghijkl').find((r) => r.id === 'non_digit')!;
    expect(mixed.ok).toBe(true);
  });

  it('hides too_long rule until it is actually violated', () => {
    const short = checkPasswordRules('a'.repeat(20)).find((r) => r.id === 'too_long')!;
    expect(short.visible).toBe(false);
    const long = checkPasswordRules('a'.repeat(1001)).find((r) => r.id === 'too_long')!;
    expect(long.visible).toBe(true);
    expect(long.ok).toBe(false);
  });

  it('passwordMeetsRules mirrors backend validate_password', () => {
    expect(passwordMeetsRules('short')).toBe(false);
    expect(passwordMeetsRules('123456789012')).toBe(false); // all digits
    expect(passwordMeetsRules('abcdefghijkl')).toBe(true);
    expect(passwordMeetsRules('brand-new-password')).toBe(true);
  });
});

describe('<AccountSettings /> change-password form', () => {
  it('disables submit while the new password fails the checklist', async () => {
    const user = userEvent.setup();
    render(<AccountSettings />);
    await waitFor(() => {
      expect(screen.getByTestId('account-email')).toHaveTextContent('vale@example.com');
    });

    await user.type(screen.getByLabelText(/current password/i), 'anything');
    await user.type(screen.getByLabelText(/new password/i), 'short');

    const submit = screen.getByRole('button', { name: /change password/i });
    expect(submit).toBeDisabled();
  });

  it('sends the request and clears fields on success', async () => {
    mockChangePassword.mockResolvedValue(undefined);
    const user = userEvent.setup();
    render(<AccountSettings />);
    await waitFor(() =>
      expect(screen.getByTestId('account-email')).toHaveTextContent('vale@example.com'),
    );

    const current = screen.getByLabelText(/current password/i) as HTMLInputElement;
    const next = screen.getByLabelText(/new password/i) as HTMLInputElement;
    await user.type(current, 'password1234');
    await user.type(next, 'brand-new-password');

    await user.click(screen.getByRole('button', { name: /change password/i }));

    await waitFor(() => {
      expect(mockChangePassword).toHaveBeenCalledWith({
        current_password: 'password1234',
        new_password: 'brand-new-password',
      });
    });
    await waitFor(() => {
      expect(screen.getByText(/password updated/i)).toBeInTheDocument();
    });
    expect(current.value).toBe('');
    expect(next.value).toBe('');
  });

  it('surfaces a 401 as "current password is incorrect"', async () => {
    mockChangePassword.mockRejectedValue(new ApiError(401, 'Current password is incorrect'));
    const user = userEvent.setup();
    render(<AccountSettings />);
    await waitFor(() =>
      expect(screen.getByTestId('account-email')).toHaveTextContent('vale@example.com'),
    );

    await user.type(screen.getByLabelText(/current password/i), 'wrong-one');
    await user.type(screen.getByLabelText(/new password/i), 'brand-new-password');
    await user.click(screen.getByRole('button', { name: /change password/i }));

    await waitFor(() => {
      expect(screen.getByText(/current password is incorrect/i)).toBeInTheDocument();
    });
  });

  it('surfaces a 429 with a friendly rate-limit message', async () => {
    mockChangePassword.mockRejectedValue(new ApiError(429, 'Too Many Requests'));
    const user = userEvent.setup();
    render(<AccountSettings />);
    await waitFor(() =>
      expect(screen.getByTestId('account-email')).toHaveTextContent('vale@example.com'),
    );

    await user.type(screen.getByLabelText(/current password/i), 'password1234');
    await user.type(screen.getByLabelText(/new password/i), 'brand-new-password');
    await user.click(screen.getByRole('button', { name: /change password/i }));

    await waitFor(() => {
      expect(screen.getByText(/too many attempts/i)).toBeInTheDocument();
    });
  });

  it('renders coming-soon tiles for change-email and delete-account', async () => {
    render(<AccountSettings />);
    await waitFor(() =>
      expect(screen.getByTestId('account-email')).toHaveTextContent('vale@example.com'),
    );

    expect(screen.getByText(/change email/i)).toBeInTheDocument();
    expect(screen.getByText(/delete account/i)).toBeInTheDocument();
  });

  it('logout button calls the auth context logout', async () => {
    const user = userEvent.setup();
    render(<AccountSettings />);
    await waitFor(() =>
      expect(screen.getByTestId('account-email')).toHaveTextContent('vale@example.com'),
    );

    await user.click(screen.getByRole('button', { name: /log out/i }));
    expect(logoutSpy).toHaveBeenCalled();
  });
});
