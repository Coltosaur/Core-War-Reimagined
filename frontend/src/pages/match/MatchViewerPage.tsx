import { useEffect } from 'react';
import { useLocation, useNavigate, useParams } from 'react-router-dom';
import { useAuth } from '../../api/useAuth';
import alertStyles from '../../components/alert.module.css';
import controls from '../../components/controls.module.css';
import gridStyles from '../../core/CoreGrid.module.css';
import { warriorText } from '../../core/warriorColors';
import { matchOutcome } from './matchOutcome';
import { useMatchReplay, type MatchStartPayload } from './useMatchReplay';
import styles from './MatchViewerPage.module.css';

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
      <div className={styles.page}>
        <p className={styles.redirect}>No active match — redirecting to lobby…</p>
      </div>
    );
  }

  const matchIdShort = params.matchId?.slice(0, 8) ?? payload.match_id.slice(0, 8);
  const outcome = matchOutcome(
    payload.result,
    payload.red_username,
    payload.blue_username,
    user?.username,
  );

  return (
    <div className={styles.page}>
      <h1 className={styles.title}>Match {matchIdShort}</h1>
      <p className={styles.versus}>
        <strong style={{ color: warriorText(0) }}>{payload.red_username}</strong> vs{' '}
        <strong style={{ color: warriorText(1) }}>{payload.blue_username}</strong>
      </p>

      {parseError && (
        <div role="alert" className={alertStyles.error}>
          {parseError}
        </div>
      )}

      <div className={styles.gridWrap}>
        <div ref={gridRef} className={`${gridStyles.grid} ${styles.replayGrid}`} />

        {!ready && !parseError && (
          <div className={`${styles.overlay} ${styles.loading}`}>Loading engine…</div>
        )}

        {finished && (
          <div className={`${styles.overlay} ${styles.finished}`}>
            <div role="status" className={styles.resultCard}>
              <p className={styles.headline} style={{ color: outcome.color }}>
                {outcome.text}
              </p>
              <p className={styles.steps}>{totalSteps.toLocaleString()} steps</p>
              <div className={controls.row}>
                <button
                  className={`${controls.button} ${controls.primary}`}
                  onClick={() => navigate('/lobby', { state: { autoQueue: true } })}
                >
                  Play Again
                </button>
                <button className={controls.button} onClick={() => navigate('/lobby')}>
                  Back to Lobby
                </button>
              </div>
            </div>
          </div>
        )}
      </div>

      <div className={styles.status}>
        <span>
          step {currentStep.toLocaleString()} / {totalSteps.toLocaleString()}
        </span>
        <span>{payload.steps_per_sec.toLocaleString()} steps/sec</span>
      </div>

      <div className={styles.warriors}>
        {warriors.map((w, i) => (
          <span key={i} style={{ color: warriorText(i) }}>
            {w.name} — {w.alive ? `${w.procs} proc${w.procs === 1 ? '' : 's'}` : 'dead'}
          </span>
        ))}
      </div>
    </div>
  );
}
