import {
  createContext,
  useContext,
  useState,
  useCallback,
  useEffect,
  type ReactNode,
} from 'react';
import React from 'react';
import {
  getToken as readToken,
  setToken as writeToken,
  clearToken as removeToken,
  isAuthenticated as checkAuth,
} from '../lib/auth';
import { pair as apiPair, getPublicHealth } from '../lib/api';

// ---------------------------------------------------------------------------
// Context shape
// ---------------------------------------------------------------------------

export interface AuthState {
  /** The current bearer token, or null if not authenticated. */
  token: string | null;
  /** Whether the user is currently authenticated. */
  isAuthenticated: boolean;
  /** True while the initial auth check is in progress. */
  loading: boolean;
  /** True when GET /health failed (wrong origin, daemon down, or network). */
  healthUnreachable: boolean;
  /** Pair with the agent using a pairing code. Stores the token on success. */
  pair: (code: string) => Promise<void>;
  /** Clear the stored token and sign out. */
  logout: () => void;
  /** Retry the public health check (e.g. after connection error). */
  retryHealth: () => void;
}

const AuthContext = createContext<AuthState | null>(null);

// ---------------------------------------------------------------------------
// Provider
// ---------------------------------------------------------------------------

export interface AuthProviderProps {
  children: ReactNode;
}

export function AuthProvider({ children }: AuthProviderProps) {
  const [token, setTokenState] = useState<string | null>(readToken);
  const [authenticated, setAuthenticated] = useState<boolean>(checkAuth);
  const [loading, setLoading] = useState<boolean>(!checkAuth());
  const [healthUnreachable, setHealthUnreachable] = useState(false);
  const [retryNonce, setRetryNonce] = useState(0);

  // On mount (and after logout / retry): if no token, ask gateway whether pairing is required.
  useEffect(() => {
    if (checkAuth()) {
      setHealthUnreachable(false);
      setLoading(false);
      return;
    }

    let cancelled = false;
    setLoading(true);
    setHealthUnreachable(false);

    getPublicHealth()
      .then((health) => {
        if (cancelled) return;
        if (!health.require_pairing) {
          setAuthenticated(true);
        }
      })
      .catch(() => {
        if (!cancelled) setHealthUnreachable(true);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [retryNonce]);

  // Keep state in sync if localStorage is changed in another tab
  useEffect(() => {
    const handler = (e: StorageEvent) => {
      if (e.key === 'multiclaw_token') {
        const t = readToken();
        setTokenState(t);
        setAuthenticated(t !== null && t.length > 0);
      }
    };
    window.addEventListener('storage', handler);
    return () => window.removeEventListener('storage', handler);
  }, []);

  const retryHealth = useCallback(() => {
    setRetryNonce((n) => n + 1);
  }, []);

  const pair = useCallback(async (code: string) => {
    const { token: newToken } = await apiPair(code);
    writeToken(newToken);
    setTokenState(newToken);
    setAuthenticated(true);
    setHealthUnreachable(false);
  }, []);

  const logout = useCallback((): void => {
    removeToken();
    setTokenState(null);
    setAuthenticated(false);
    setHealthUnreachable(false);
    setLoading(true);
    setRetryNonce((n) => n + 1);
  }, []);

  const value: AuthState = {
    token,
    isAuthenticated: authenticated,
    loading,
    healthUnreachable,
    pair,
    logout,
    retryHealth,
  };

  return React.createElement(AuthContext.Provider, { value }, children);
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

/**
 * Access the authentication state from any component inside `<AuthProvider>`.
 * Throws if used outside the provider.
 */
export function useAuth(): AuthState {
  const ctx = useContext(AuthContext);
  if (!ctx) {
    throw new Error('useAuth must be used within an <AuthProvider>');
  }
  return ctx;
}
