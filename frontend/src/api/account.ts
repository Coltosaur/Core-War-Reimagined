import { api } from './client';

export type Account = {
  email: string;
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
