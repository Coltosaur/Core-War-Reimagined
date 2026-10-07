import { useContext } from 'react';
import { AuthContext } from './authContextValue';
import type { AuthState } from './authContextValue';

export function useAuth(): AuthState {
  const ctx = useContext(AuthContext);
  if (!ctx) throw new Error('useAuth must be used within AuthProvider');
  return ctx;
}
