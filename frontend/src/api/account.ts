import { api } from './client';

export type Account = {
  // `null` when the user registered without an email (email is optional
  // until #83 ships).
  email: string | null;
};

export type ChangePasswordInput = {
  current_password: string;
  new_password: string;
};

export async function getAccount(): Promise<Account> {
  return api.get<Account>('/api/account');
}

export async function changePassword(input: ChangePasswordInput): Promise<void> {
  return api.post('/api/auth/change-password', input);
}
