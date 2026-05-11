import { useEffect, useRef, useState } from 'react';
import { useAuth } from '../api/AuthContext';
import { useWarriorLibrary, type Warrior } from '../warriors/library';
import { io, type Socket } from 'socket.io-client';

const API_BASE = import.meta.env.VITE_API_URL ?? 'http://localhost:3001';

function serverUuid(w: Warrior): string | null {
  return w.id.startsWith('server:') ? w.id.slice(7) : null;
}

const PAGE_STYLE: React.CSSProperties = {
  minHeight: '100vh',
  display: 'flex',
  flexDirection: 'column',
  alignItems: 'center',
  justifyContent: 'center',
  padding: '2rem',
  gap: '1.5rem',
};

const TITLE_STYLE: React.CSSProperties = {
  margin: 0,
  fontSize: '1.8rem',
  color: '#e94560',
  letterSpacing: '0.08em',
};

const SELECT_STYLE: React.CSSProperties = {
  padding: '0.5rem 0.75rem',
  fontSize: '0.85rem',
  fontFamily: 'inherit',
  backgroundColor: '#111',
  color: '#e0e0e0',
  border: '1px solid #333',
  borderRadius: '4px',
  minWidth: '200px',
};

const BTN_STYLE: React.CSSProperties = {
  padding: '0.75rem 2rem',
  fontSize: '1rem',
  fontFamily: 'inherit',
  letterSpacing: '0.05em',
  border: 'none',
  borderRadius: '6px',
  cursor: 'pointer',
  transition: 'background-color 0.15s',
};

const QUEUE_BTN: React.CSSProperties = {
  ...BTN_STYLE,
  backgroundColor: '#e94560',
  color: '#fff',
};

const CANCEL_BTN: React.CSSProperties = {
  ...BTN_STYLE,
  backgroundColor: '#333',
  color: '#e0e0e0',
};

const STATUS_STYLE: React.CSSProperties = {
  color: '#888',
  fontSize: '0.85rem',
};

const RESULT_BOX: React.CSSProperties = {
  padding: '1.5rem',
  backgroundColor: '#111',
  border: '1px solid #333',
  borderRadius: '8px',
  textAlign: 'center',
  minWidth: '300px',
};

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
  const library = useWarriorLibrary();
  const userWarriors = library.filter((w: Warrior) => serverUuid(w) !== null);
  const firstUuid = userWarriors.length > 0 ? (serverUuid(userWarriors[0]) ?? '') : '';
  const [selectedId, setSelectedId] = useState('');
  const [phase, setPhase] = useState<Phase>('idle');
  const [matchInfo, setMatchInfo] = useState<MatchFoundData | null>(null);
  const [result, setResult] = useState<MatchResultData | null>(null);
  const [error, setError] = useState<string | null>(null);
  const socketRef = useRef<Socket | null>(null);

  const effectiveId = selectedId || firstUuid;

  function connect() {
    if (socketRef.current) return socketRef.current;
    const s = io(API_BASE, { withCredentials: true });
    socketRef.current = s;

    s.on('queue:joined', () => setPhase('queued'));
    s.on('queue:left', () => setPhase('idle'));
    s.on('queue:error', (data: { error: string }) => {
      setError(data.error);
      setPhase('idle');
    });
    s.on('match:found', (data: MatchFoundData) => {
      setMatchInfo(data);
      setPhase('matched');
    });
    s.on('match:result', (data: MatchResultData) => {
      setResult(data);
      setPhase('result');
    });

    return s;
  }

  function joinQueue() {
    if (!effectiveId) return;
    setError(null);
    const s = connect();
    s.emit('queue:join', { warrior_id: effectiveId });
  }

  function leaveQueue() {
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
      socketRef.current?.disconnect();
    };
  }, []);

  if (!user) {
    return (
      <div style={PAGE_STYLE}>
        <p style={{ color: '#888' }}>Log in to play ranked matches.</p>
      </div>
    );
  }

  if (userWarriors.length === 0) {
    return (
      <div style={PAGE_STYLE}>
        <h1 style={TITLE_STYLE}>Ranked Play</h1>
        <p style={{ color: '#888' }}>Save a warrior in the Builder to enter matchmaking.</p>
      </div>
    );
  }

  return (
    <div style={PAGE_STYLE}>
      <h1 style={TITLE_STYLE}>Ranked Play</h1>

      {error && <p style={{ color: '#e94560' }}>{error}</p>}

      {phase === 'idle' && (
        <>
          <label style={{ color: '#888', fontSize: '0.75rem' }}>Choose your warrior</label>
          <select
            style={SELECT_STYLE}
            value={effectiveId}
            onChange={(e) => setSelectedId(e.target.value)}
          >
            {userWarriors.map((w: Warrior) => (
              <option key={w.id} value={serverUuid(w) ?? ''}>
                {w.label}
              </option>
            ))}
          </select>
          <button style={QUEUE_BTN} onClick={joinQueue}>
            Find Match
          </button>
        </>
      )}

      {phase === 'queued' && (
        <>
          <p style={STATUS_STYLE}>Searching for opponent...</p>
          <button style={CANCEL_BTN} onClick={leaveQueue}>
            Cancel
          </button>
        </>
      )}

      {phase === 'matched' && matchInfo && (
        <div style={RESULT_BOX}>
          <p style={{ color: '#f0c040', fontSize: '1.1rem', fontWeight: 600 }}>Match Found!</p>
          <p style={{ color: '#e0e0e0' }}>
            {matchInfo.red_username} vs {matchInfo.blue_username}
          </p>
          <p style={STATUS_STYLE}>Running battle...</p>
        </div>
      )}

      {phase === 'result' && result && (
        <div style={RESULT_BOX}>
          <p
            style={{
              fontSize: '1.3rem',
              fontWeight: 700,
              color: resultColor(
                result.result,
                result.red_username,
                result.blue_username,
                user.username,
              ),
            }}
          >
            {resultLabel(result.result, result.red_username, result.blue_username, user.username)}
          </p>
          <p style={{ color: '#e0e0e0', margin: '0.5rem 0' }}>
            {result.red_username} vs {result.blue_username}
          </p>
          <p style={STATUS_STYLE}>{result.steps_taken.toLocaleString()} steps</p>
          <button style={{ ...QUEUE_BTN, marginTop: '1rem' }} onClick={resetLobby}>
            Play Again
          </button>
        </div>
      )}
    </div>
  );
}

function resultLabel(result: string, red: string, blue: string, me: string): string {
  if (result === 'tie' || result === 'all_dead') return 'Draw';
  const winner = result === 'red_win' ? red : blue;
  return winner === me ? 'Victory!' : 'Defeat';
}

function resultColor(result: string, red: string, blue: string, me: string): string {
  if (result === 'tie' || result === 'all_dead') return '#f0c040';
  const winner = result === 'red_win' ? red : blue;
  return winner === me ? '#4caf50' : '#e94560';
}
