import { useEffect } from 'react';
import { useLocation, useNavigate, useParams } from 'react-router-dom';
import { useAuth } from '../../api/AuthContext';
import { GRID_CONTAINER_STYLE } from '../battlefield/styles';
import { useMatchReplay, type MatchStartPayload } from './useMatchReplay';

const PAGE_STYLE: React.CSSProperties = {
  minHeight: '100vh',
  display: 'flex',
  flexDirection: 'column',
  alignItems: 'center',
  padding: '1.5rem',
  gap: '1rem',
};

const TITLE_STYLE: React.CSSProperties = {
  margin: 0,
  fontSize: '1.6rem',
  color: '#e94560',
  letterSpacing: '0.08em',
};

const VERSUS_STYLE: React.CSSProperties = {
  margin: 0,
  fontSize: '1rem',
  color: '#e0e0e0',
  letterSpacing: '0.04em',
};

const STATUS_ROW: React.CSSProperties = {
  display: 'flex',
  gap: '2rem',
  fontSize: '0.85rem',
  color: '#bbb',
  fontFamily: '"JetBrains Mono", "Fira Code", monospace',
};

const RESULT_BOX: React.CSSProperties = {
  padding: '1rem 1.5rem',
  backgroundColor: '#111',
  border: '1px solid #333',
  borderRadius: '8px',
  textAlign: 'center',
  minWidth: '260px',
};

const WARRIOR_ROW: React.CSSProperties = {
  display: 'flex',
  gap: '2rem',
  fontSize: '0.85rem',
  fontFamily: '"JetBrains Mono", "Fira Code", monospace',
};

function resultLabel(result: string, red: string, blue: string, me: string | undefined): string {
  if (result === 'tie' || result === 'all_dead') return 'Draw';
  const winner = result === 'red_win' ? red : blue;
  if (!me) return `${winner} wins`;
  return winner === me ? 'Victory!' : 'Defeat';
}

function resultColor(result: string, red: string, blue: string, me: string | undefined): string {
  if (result === 'tie' || result === 'all_dead') return '#f0c040';
  const winner = result === 'red_win' ? red : blue;
  if (!me) return '#e0e0e0';
  return winner === me ? '#4caf50' : '#e94560';
}

export default function MatchViewerPage() {
  const { user } = useAuth();
  const navigate = useNavigate();
  const location = useLocation();
  const params = useParams<{ matchId: string }>();

  // Match payload comes from the lobby via router state. Reload-without-state
  // bounces back to the lobby (spectator-join via deep-link is PR 4 territory).
  const payload = (location.state as { matchStart?: MatchStartPayload } | null)?.matchStart ?? null;

  useEffect(() => {
    if (!payload) {
      navigate('/lobby', { replace: true });
    }
  }, [payload, navigate]);

  const { ready, currentStep, totalSteps, warriors, finished, parseError, gridRef } =
    useMatchReplay(payload);

  if (!payload) {
    return (
      <div style={PAGE_STYLE}>
        <p style={{ color: '#888' }}>No active match — redirecting to lobby…</p>
      </div>
    );
  }

  const matchIdShort = params.matchId?.slice(0, 8) ?? payload.match_id.slice(0, 8);

  return (
    <div style={PAGE_STYLE}>
      <h1 style={TITLE_STYLE}>MATCH {matchIdShort}</h1>
      <p style={VERSUS_STYLE}>
        {payload.red_username} vs {payload.blue_username}
      </p>

      {parseError && <div style={{ color: '#e94560' }}>{parseError}</div>}

      <div ref={gridRef} style={GRID_CONTAINER_STYLE} />

      <div style={STATUS_ROW}>
        <span>
          step {currentStep.toLocaleString()} / {totalSteps.toLocaleString()}
        </span>
        <span>{payload.steps_per_sec.toLocaleString()} steps/sec</span>
      </div>

      <div style={WARRIOR_ROW}>
        {warriors.map((w, i) => (
          <span key={i} style={{ color: i === 0 ? '#e94560' : '#4fc3f7' }}>
            {w.name} — {w.alive ? `${w.procs} proc${w.procs === 1 ? '' : 's'}` : 'dead'}
          </span>
        ))}
      </div>

      {finished && (
        <div style={RESULT_BOX}>
          <p
            style={{
              fontSize: '1.3rem',
              fontWeight: 700,
              margin: 0,
              color: resultColor(
                payload.result,
                payload.red_username,
                payload.blue_username,
                user?.username,
              ),
            }}
          >
            {resultLabel(
              payload.result,
              payload.red_username,
              payload.blue_username,
              user?.username,
            )}
          </p>
          <p style={{ color: '#888', fontSize: '0.85rem', margin: '0.5rem 0 1rem' }}>
            {totalSteps.toLocaleString()} steps
          </p>
          <button
            style={{
              padding: '0.5rem 1.5rem',
              backgroundColor: '#e94560',
              color: '#fff',
              border: 'none',
              borderRadius: '6px',
              cursor: 'pointer',
              fontFamily: 'inherit',
              letterSpacing: '0.05em',
            }}
            onClick={() => navigate('/lobby')}
          >
            Back to Lobby
          </button>
        </div>
      )}

      {!ready && !parseError && <p style={{ color: '#888' }}>Loading engine…</p>}
    </div>
  );
}
