import { useCallback, useEffect, useRef, useState } from 'react';
import { useLocation, useNavigate } from 'react-router-dom';
import { useAuth } from '../api/useAuth';
import { installSocketAuthRecovery, type PendingSocketAction } from '../api/socketAuth';
import { useWarriorLibrary, type Warrior } from '../warriors/library';
import { io, type Socket } from 'socket.io-client';
import alertStyles from '../components/alert.module.css';
import controls from '../components/controls.module.css';
import { matchOutcome } from './match/matchOutcome';
import type { MatchStartPayload } from './match/useMatchReplay';
import styles from './LobbyPage.module.css';

const API_BASE = import.meta.env.VITE_API_URL ?? 'http://localhost:3001';

function serverUuid(w: Warrior): string | null {
  return w.id.startsWith('server:') ? w.id.slice(7) : null;
}

type MatchFoundData = {
  match_id: string;
  red_username: string;
  blue_username: string;
};

type MatchResultData = {
  match_id: string;
  result: string;
  steps_taken: number;
  red_username: string;
  blue_username: string;
};

type Phase = 'idle' | 'queued' | 'matched' | 'result';

export default function LobbyPage() {
  const { user } = useAuth();
  const navigate = useNavigate();
  const location = useLocation();
  const library = useWarriorLibrary();
  const userWarriors = library.filter((w: Warrior) => serverUuid(w) !== null);
  const firstUuid = userWarriors.length > 0 ? (serverUuid(userWarriors[0]) ?? '') : '';
  const [selectedId, setSelectedId] = useState('');
  const [phase, setPhase] = useState<Phase>('idle');
  const [matchInfo, setMatchInfo] = useState<MatchFoundData | null>(null);
  const [result, setResult] = useState<MatchResultData | null>(null);
  const [error, setError] = useState<string | null>(null);
  const socketRef = useRef<Socket | null>(null);
  const socketTeardownRef = useRef<(() => void) | null>(null);
  const pendingActionRef = useRef<PendingSocketAction | null>(null);
  const autoQueuedRef = useRef(false);

  const effectiveId = selectedId || firstUuid;

  const connect = useCallback(() => {
    if (socketRef.current) return socketRef.current;
    const s = io(API_BASE, { withCredentials: true });
    socketRef.current = s;

    // pendingActionRef stays set while queued, not just until the server
    // acknowledges: the server drops a connection's queue entry when it
    // disconnects, and the auth-recovery layer replays the pending action on
    // every reconnect to put the player back (#127).
    s.on('queue:joined', () => {
      setPhase('queued');
    });
    s.on('queue:left', () => {
      pendingActionRef.current = null;
      setPhase('idle');
    });
    s.on('queue:error', (data: { error: string }) => {
      pendingActionRef.current = null;
      setError(data.error);
      setPhase('idle');
    });
    s.on('match:found', (data: MatchFoundData) => {
      pendingActionRef.current = null;
      setMatchInfo(data);
      setPhase('matched');
    });
    s.on('match:start', (data: MatchStartPayload) => {
      // Hand off to the dedicated viewer route with the full replay payload.
      // The lobby's socket gets cleaned up on unmount; the viewer doesn't need
      // one because match:start carries everything for deterministic replay.
      navigate(`/match/${data.match_id}`, { state: { matchStart: data } });
    });
    s.on('match:result', (data: MatchResultData) => {
      setResult(data);
      setPhase('result');
    });

    // Silent recovery on access-token expiry and on reconnects. Without
    // this, an idle 15+ minute session lets the socket reconnect as
    // anonymous and `require_auth` on the backend drops queue:join with no
    // visible feedback (issue #60), and a reconnect while queued leaves the
    // page showing "Searching" for a queue entry that no longer exists (#127).
    socketTeardownRef.current = installSocketAuthRecovery(s, {
      getPendingAction: () => pendingActionRef.current,
      onRecovered: () => setError(null),
      onSessionExpired: () => {
        pendingActionRef.current = null;
        setPhase('idle');
        setError('Your session has expired. Please log in again.');
      },
    });

    return s;
  }, [navigate]);

  // Takes the warrior id as an argument (rather than closing over the derived
  // `effectiveId`) so the callback's deps stay stable values only.
  const joinQueue = useCallback(
    (warriorId: string) => {
      if (!warriorId) return;
      setError(null);
      const s = connect();
      const payload = { warrior_id: warriorId };
      // Record the pending action *before* emitting so the socket-auth
      // recovery layer can replay it if the backend drops this emit because
      // the connection is anonymous.
      pendingActionRef.current = { event: 'queue:join', payload };
      s.emit('queue:join', payload);
    },
    [connect],
  );

  // "Play Again" from the match viewer navigates here with { autoQueue: true }.
  // Re-join the queue once a warrior is available, then clear the router state
  // so a refresh or back-navigation doesn't silently re-queue. The latch is
  // released on cleanup so StrictMode's mount/cleanup/mount cycle (which tears
  // down the first socket) re-establishes the connection on the second mount.
  useEffect(() => {
    const autoQueue = (location.state as { autoQueue?: boolean } | null)?.autoQueue;
    if (!autoQueue || autoQueuedRef.current || !effectiveId) return;
    autoQueuedRef.current = true;
    joinQueue(effectiveId);
    navigate(location.pathname, { replace: true });
    return () => {
      autoQueuedRef.current = false;
    };
  }, [location.state, location.pathname, effectiveId, joinQueue, navigate]);

  function leaveQueue() {
    pendingActionRef.current = null;
    socketRef.current?.emit('queue:leave');
    setPhase('idle');
  }

  function resetLobby() {
    setPhase('idle');
    setMatchInfo(null);
    setResult(null);
    setError(null);
  }

  useEffect(() => {
    return () => {
      socketTeardownRef.current?.();
      socketTeardownRef.current = null;
      socketRef.current?.disconnect();
      socketRef.current = null;
      pendingActionRef.current = null;
    };
  }, []);

  if (!user) {
    return (
      <div className={styles.page}>
        <h1 className={styles.title}>Ranked Play</h1>
        <p className={styles.hint}>Log in to play ranked matches.</p>
      </div>
    );
  }

  if (userWarriors.length === 0) {
    return (
      <div className={styles.page}>
        <h1 className={styles.title}>Ranked Play</h1>
        <p className={styles.hint}>Save a warrior in the Builder to enter matchmaking.</p>
      </div>
    );
  }

  const outcome =
    result && matchOutcome(result.result, result.red_username, result.blue_username, user.username);

  return (
    <div className={styles.page}>
      <h1 className={styles.title}>Ranked Play</h1>

      {error && (
        <p role="alert" className={alertStyles.error}>
          {error}
        </p>
      )}

      {phase === 'idle' && (
        <div className={styles.picker}>
          <label htmlFor="lobby-warrior" className={styles.label}>
            Choose your warrior
          </label>
          <select
            id="lobby-warrior"
            className={controls.select}
            value={effectiveId}
            onChange={(e) => setSelectedId(e.target.value)}
          >
            {userWarriors.map((w: Warrior) => (
              <option key={w.id} value={serverUuid(w) ?? ''}>
                {w.label}
              </option>
            ))}
          </select>
          <button
            className={`${controls.button} ${controls.primary}`}
            onClick={() => joinQueue(effectiveId)}
          >
            Find Match
          </button>
        </div>
      )}

      {phase === 'queued' && (
        <>
          <p role="status" className={styles.status}>
            Searching for opponent...
          </p>
          <button className={controls.button} onClick={leaveQueue}>
            Cancel
          </button>
        </>
      )}

      {phase === 'matched' && matchInfo && (
        <div role="status" className={styles.card}>
          <p className={styles.found}>Match Found!</p>
          <p className={styles.versus}>
            {matchInfo.red_username} vs {matchInfo.blue_username}
          </p>
          <p className={styles.status}>Running battle...</p>
        </div>
      )}

      {phase === 'result' && result && outcome && (
        <div role="status" className={styles.card}>
          <p className={styles.headline} style={{ color: outcome.color }}>
            {outcome.text}
          </p>
          <p className={styles.versus}>
            {result.red_username} vs {result.blue_username}
          </p>
          <p className={styles.status}>{result.steps_taken.toLocaleString()} steps</p>
          <button
            className={`${controls.button} ${controls.primary} ${styles.again}`}
            onClick={resetLobby}
          >
            Play Again
          </button>
        </div>
      )}
    </div>
  );
}
