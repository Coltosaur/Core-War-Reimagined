import { api, ApiError } from './client';

export type AuthUser = {
  user_id: string;
  username: string;
};

export type RegisterInput = {
  username: string;
  email: string;
  password: string;
};

export type LoginInput = {
  username_or_email: string;
  password: string;
};

export async function register(input: RegisterInput): Promise<AuthUser> {
  return api.post<AuthUser>('/api/auth/register', input);
}

export async function login(input: LoginInput): Promise<AuthUser> {
  return api.post<AuthUser>('/api/auth/login', input);
}

export async function logout(): Promise<void> {
  return api.post('/api/auth/logout');
}

export async function getMe(): Promise<AuthUser | null> {
  try {
    return await api.get<AuthUser>('/api/auth/me');
  } catch (e) {
    if (e instanceof ApiError && e.status === 401) return null;
    throw e;
  }
}

export async function refreshToken(): Promise<AuthUser> {
  return api.post<AuthUser>('/api/auth/refresh');
}
