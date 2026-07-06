import { createContext, useCallback, useContext, useEffect, useState } from 'react';
import type { ReactNode } from 'react';
import * as authApi from './auth';
import { registerSessionHandlers } from './session';

type AuthState = {
  user: authApi.AuthUser | null;
  loading: boolean;
  login: (input: authApi.LoginInput) => Promise<void>;
  register: (input: authApi.RegisterInput) => Promise<void>;
  logout: () => Promise<void>;
};

const AuthContext = createContext<AuthState | null>(null);

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<authApi.AuthUser | null>(null);
  const [loading, setLoading] = useState(true);

  // Wire session.ts's forceLogout() to setUser(null) so the REST interceptor
  // (client.ts) and any socket-auth recovery flow (socketAuth.ts) can reset
  // the UI without needing to route through a component ref or a global
  // event bus. Registered inside a useEffect so the setUser closure is
  // recreated after every remount instead of pinning a stale one.
  useEffect(() => {
    return registerSessionHandlers({
      onForceLogout: () => setUser(null),
    });
  }, []);

  useEffect(() => {
    authApi
      .getMe()
      .then(setUser)
      .catch(() => setUser(null))
      .finally(() => setLoading(false));
  }, []);

  const login = useCallback(async (input: authApi.LoginInput) => {
    const u = await authApi.login(input);
    setUser(u);
  }, []);

  const register = useCallback(async (input: authApi.RegisterInput) => {
    const u = await authApi.register(input);
    setUser(u);
  }, []);

  const logout = useCallback(async () => {
    await authApi.logout();
    setUser(null);
  }, []);

  return (
    <AuthContext.Provider value={{ user, loading, login, register, logout }}>
      {children}
    </AuthContext.Provider>
  );
}

export function useAuth(): AuthState {
  const ctx = useContext(AuthContext);
  if (!ctx) throw new Error('useAuth must be used within AuthProvider');
  return ctx;
}
