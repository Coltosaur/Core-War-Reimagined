import { useEffect } from 'react';
import { useAuth } from '../api/AuthContext';
import { clearServerWarriors, setServerOwner, syncFromServer } from './library';

/**
 * App-level owner of the server warrior cache. Mounted once in AppLayout so
 * every route — not just the Builder — hydrates the logged-in user's
 * warriors:
 *   - while auth is still resolving, the persisted cache is shown as-is
 *     (stale-while-revalidate);
 *   - once a user is known, the cache is pinned to that user_id and
 *     revalidated against the server;
 *   - on logout / forced logout, the cache is dropped.
 */
export function useServerWarriorSync(): void {
  const { user, loading } = useAuth();
  const userId = user?.user_id ?? null;

  useEffect(() => {
    if (loading) return;
    if (!userId) {
      clearServerWarriors();
      return;
    }
    setServerOwner(userId);
    void syncFromServer();
  }, [userId, loading]);
}
