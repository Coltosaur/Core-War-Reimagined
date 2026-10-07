import { createContext } from 'react';
import type * as authApi from './auth';

export type AuthState = {
  user: authApi.AuthUser | null;
  loading: boolean;
  login: (input: authApi.LoginInput) => Promise<void>;
  register: (input: authApi.RegisterInput) => Promise<void>;
  logout: () => Promise<void>;
};

// Kept out of AuthContext.tsx so that file exports only the AuthProvider
// component, which React Fast Refresh requires. AuthProvider supplies the
// value; useAuth (./useAuth.ts) reads it.
export const AuthContext = createContext<AuthState | null>(null);
