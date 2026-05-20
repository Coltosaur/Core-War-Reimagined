import { useEffect } from 'react';
import { useLocation, useNavigate, useParams } from 'react-router-dom';
import { useAuth } from '../../api/AuthContext';
import { GRID_CONTAINER_STYLE, PARSE_ERROR_STYLE } from '../battlefield/styles';
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

const WARRIOR_ROW: React.CSSProperties = {
  display: 'flex',
  gap: '2rem',
  fontSize: '0.85rem',
  fontFamily: '"JetBrains Mono", "Fira Code", monospace',
};

// Wrap the imperative PixiJS canvas so overlays can be positioned over it.
// Overlays are siblings of the canvas container (never children of gridRef),
// so React never reconciles away the canvas Pixi appends imperatively.
const GRID_WRAP_STYLE: React.CSSProperties = {
  position: 'relative',
  display: 'inline-block',
  lineHeight: 0,
};

const OVERLAY_BASE: React.CSSProperties = {
  position: 'absolute',
  inset: 0,
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
  lineHeight: 'normal',
};

const LOADING_OVERLAY_STYLE: React.CSSProperties = {
  ...OVERLAY_BASE,
  backgroundColor: 'rgba(10, 10, 12, 0.8)',
  color: '#888',
  fontSize: '0.9rem',
  letterSpacing: '0.05em',
};

const RESULT_CARD_STYLE: React.CSSProperties = {
  padding: '1.5rem 2rem',
  backgroundColor: '#111',
  border: '1px solid #333',
  borderRadius: '10px',
  textAlign: 'center',
  minWidth: '240px',
  boxShadow: '0 12px 40px rgba(0, 0, 0, 0.55)',
};

const RESULT_HEADLINE_STYLE: React.CSSProperties = {
  fontSize: '1.6rem',
  fontWeight: 700,
  margin: 0,
};

const RESULT_SUB_STYLE: React.CSSProperties = {
  color: '#888',
  fontSize: '0.85rem',
  margin: '0.4rem 0 1.2rem',
};

const BUTTON_ROW_STYLE: React.CSSProperties = {
  display: 'flex',
  gap: '0.6rem',
  justifyContent: 'center',
};

const PRIMARY_BTN_STYLE: React.CSSProperties = {
  padding: '0.5rem 1.3rem',
  backgroundColor: '#e94560',
  color: '#fff',
  border: 'none',
  borderRadius: '6px',
  cursor: 'pointer',
  fontFamily: 'inherit',
  fontSize: '0.9rem',
  letterSpacing: '0.04em',
};

const SECONDARY_BTN_STYLE: React.CSSProperties = {
  padding: '0.5rem 1.3rem',
  backgroundColor: 'transparent',
  color: '#bbb',
  border: '1px solid #444',
  borderRadius: '6px',
  cursor: 'pointer',
  fontFamily: 'inherit',
  fontSize: '0.9rem',
  letterSpacing: '0.04em',
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
  // bounces back to the lobby (spectator-join via deep-link is issue #51).
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

      {parseError && <div style={PARSE_ERROR_STYLE}>{parseError}</div>}

      <div style={GRID_WRAP_STYLE}>
        <div ref={gridRef} style={GRID_CONTAINER_STYLE} />

        {!ready && !parseError && <div style={LOADING_OVERLAY_STYLE}>Loading engine…</div>}

        {finished && (
          <div
            style={{
              ...OVERLAY_BASE,
              backgroundColor: 'rgba(10, 10, 12, 0.72)',
              animation: 'cw-fade-in 200ms ease-out both',
            }}
          >
            <div
              style={{
                ...RESULT_CARD_STYLE,
                animation: 'cw-result-pop 260ms cubic-bezier(0.2, 0.9, 0.3, 1.2) both',
              }}
            >
              <p
                style={{
                  ...RESULT_HEADLINE_STYLE,
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
              <p style={RESULT_SUB_STYLE}>{totalSteps.toLocaleString()} steps</p>
              <div style={BUTTON_ROW_STYLE}>
                <button
                  style={PRIMARY_BTN_STYLE}
                  onClick={() => navigate('/lobby', { state: { autoQueue: true } })}
                >
                  Play Again
                </button>
                <button style={SECONDARY_BTN_STYLE} onClick={() => navigate('/lobby')}>
                  Back to Lobby
                </button>
              </div>
            </div>
          </div>
        )}
      </div>

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
    </div>
  );
}
