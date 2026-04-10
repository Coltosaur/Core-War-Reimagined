import { useCallback, useEffect, useRef, useState } from 'react';
import init, {
  parseWarrior,
  MatchState,
  engineVersion,
  type ParsedWarrior,
} from 'core-war-engine';
import { createCoreRenderer, type CoreRenderer } from './core/coreRenderer';
import { CORE_SIZE } from './core/constants';

// ─── Built-in warrior sources for the demo battle ───────────────────

const IMP_SOURCE = `
;name Imp
        MOV.I $0, $1
`;

const DWARF_SOURCE = `
;name Dwarf
;author A.K. Dewdney
        ORG    start
start   ADD.AB #4, bomb
        MOV.I  bomb, @bomb
        JMP    start
bomb    DAT.F  #0, #0
`;

// ─── Styles (inline for now — extract to CSS when the UI grows) ─────

const ROOT_STYLE: React.CSSProperties = {
  fontFamily: '"JetBrains Mono", "Fira Code", monospace',
  backgroundColor: '#0a0a0a',
  color: '#e0e0e0',
  minHeight: '100vh',
  display: 'flex',
  flexDirection: 'column',
  alignItems: 'center',
  padding: '1.5rem',
  gap: '1rem',
};

const GRID_CONTAINER_STYLE: React.CSSProperties = {
  border: '1px solid #333',
  lineHeight: 0,
};

const CONTROLS_STYLE: React.CSSProperties = {
  display: 'flex',
  gap: '0.5rem',
  alignItems: 'center',
  flexWrap: 'wrap',
  justifyContent: 'center',
};

const BUTTON_STYLE: React.CSSProperties = {
  padding: '0.4rem 1rem',
  fontFamily: 'inherit',
  fontSize: '0.85rem',
  cursor: 'pointer',
  backgroundColor: '#1e1e1e',
  color: '#e0e0e0',
  border: '1px solid #444',
  borderRadius: '4px',
};

const STATUS_STYLE: React.CSSProperties = {
  fontSize: '0.85rem',
  color: '#888',
  textAlign: 'center',
};

// ─── Result code constants (match the wasm API) ─────────────────────

const ONGOING = 0;
const VICTORY = 1;
const TIE = 2;
const ALL_DEAD = 3;

function resultText(code: number, winnerId: number): string {
  switch (code) {
    case ONGOING:
      return 'Battle in progress';
    case VICTORY:
      return `Warrior ${winnerId} wins!`;
    case TIE:
      return 'Tie — step limit reached';
    case ALL_DEAD:
      return 'All warriors dead';
    default:
      return '';
  }
}

// ─── App ─────────────────────────────────────────────────────────────

export default function App() {
  const [ready, setReady] = useState(false);
  const [running, setRunning] = useState(false);
  const [stepCount, setStepCount] = useState(0);
  const [resultCode, setResultCode] = useState(ONGOING);
  const [resultWinner, setResultWinner] = useState(-1);
  const [stepsPerFrame, setStepsPerFrame] = useState(50);
  const [warriors, setWarriors] = useState<
    { name: string; alive: boolean; procs: number }[]
  >([]);

  const gridRef = useRef<HTMLDivElement>(null);
  const rendererRef = useRef<CoreRenderer | null>(null);
  const matchRef = useRef<MatchState | null>(null);
  const rafRef = useRef(0);
  const frameCountRef = useRef(0);
  const spfRef = useRef(stepsPerFrame);

  // Keep the ref in sync so the rAF callback sees the latest value
  // without needing to be recreated.
  spfRef.current = stepsPerFrame;

  // ── Init wasm + PixiJS ────────────────────────────────────────────

  useEffect(() => {
    let cancelled = false;
    init().then(() => {
      if (!cancelled) setReady(true);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (!gridRef.current) return;
    const renderer = createCoreRenderer(gridRef.current);
    rendererRef.current = renderer;
    return () => {
      renderer.destroy();
      rendererRef.current = null;
    };
  }, []);

  // ── Battle lifecycle ──────────────────────────────────────────────

  const loadBattle = useCallback(() => {
    if (!ready) return;

    // Clean up previous battle
    cancelAnimationFrame(rafRef.current);
    setRunning(false);

    const match = new MatchState(CORE_SIZE, 80_000);

    let w1: ParsedWarrior, w2: ParsedWarrior;
    try {
      w1 = parseWarrior(IMP_SOURCE);
      w2 = parseWarrior(DWARF_SOURCE);
    } catch (e) {
      console.error('Parse error:', e);
      return;
    }

    match.loadWarrior(0, w1, 0);
    match.loadWarrior(1, w2, Math.floor(CORE_SIZE / 2));
    matchRef.current = match;

    // Show the initial (empty) core
    if (rendererRef.current) {
      rendererRef.current.update(match.coreOpcodes());
    }

    setStepCount(0);
    setResultCode(ONGOING);
    setResultWinner(-1);
    setWarriors([
      { name: w1.name() ?? 'Warrior 0', alive: true, procs: 1 },
      { name: w2.name() ?? 'Warrior 1', alive: true, procs: 1 },
    ]);
  }, [ready]);

  // Load the default battle once wasm is ready.
  useEffect(() => {
    if (ready) loadBattle();
  }, [ready, loadBattle]);

  // ── Animation loop ────────────────────────────────────────────────

  const syncUiState = useCallback(() => {
    const m = matchRef.current;
    if (!m) return;
    setStepCount(m.steps());
    const code = m.resultCode();
    setResultCode(code);
    setResultWinner(m.resultWinnerId());
    const ws: { name: string; alive: boolean; procs: number }[] = [];
    for (let i = 0; i < m.warriorCount(); i++) {
      ws.push({
        name: `Warrior ${m.warriorId(i)}`,
        alive: m.warriorIsAlive(i),
        procs: m.warriorProcessCount(i),
      });
    }
    setWarriors(ws);
  }, []);

  const tick = useCallback(() => {
    const m = matchRef.current;
    const r = rendererRef.current;
    if (!m || !r) return;

    m.stepN(spfRef.current);
    r.update(m.coreOpcodes());

    // Throttle React state updates to every 6 frames (~10Hz at 60fps).
    frameCountRef.current++;
    if (frameCountRef.current % 6 === 0) {
      syncUiState();
    }

    if (m.resultCode() === ONGOING) {
      rafRef.current = requestAnimationFrame(tick);
    } else {
      syncUiState();
      setRunning(false);
    }
  }, [syncUiState]);

  // ── Controls ──────────────────────────────────────────────────────

  const play = useCallback(() => {
    if (!matchRef.current) return;
    if (matchRef.current.resultCode() !== ONGOING) return;
    setRunning(true);
    frameCountRef.current = 0;
    rafRef.current = requestAnimationFrame(tick);
  }, [tick]);

  const pause = useCallback(() => {
    cancelAnimationFrame(rafRef.current);
    setRunning(false);
    syncUiState();
  }, [syncUiState]);

  const stepOnce = useCallback(() => {
    const m = matchRef.current;
    const r = rendererRef.current;
    if (!m || !r || m.resultCode() !== ONGOING) return;
    m.stepN(1);
    r.update(m.coreOpcodes());
    syncUiState();
  }, [syncUiState]);

  const stepMany = useCallback(() => {
    const m = matchRef.current;
    const r = rendererRef.current;
    if (!m || !r || m.resultCode() !== ONGOING) return;
    m.stepN(100);
    r.update(m.coreOpcodes());
    syncUiState();
  }, [syncUiState]);

  const reset = useCallback(() => {
    cancelAnimationFrame(rafRef.current);
    setRunning(false);
    loadBattle();
  }, [loadBattle]);

  // ── Render ────────────────────────────────────────────────────────

  return (
    <div style={ROOT_STYLE}>
      <h1 style={{ margin: 0, fontSize: '1.5rem', letterSpacing: '0.1em' }}>
        CORE WAR
      </h1>

      <div ref={gridRef} style={GRID_CONTAINER_STYLE} />

      <div style={CONTROLS_STYLE}>
        {!running ? (
          <button style={BUTTON_STYLE} onClick={play}>
            Play
          </button>
        ) : (
          <button style={BUTTON_STYLE} onClick={pause}>
            Pause
          </button>
        )}
        <button
          style={BUTTON_STYLE}
          onClick={stepOnce}
          disabled={running || resultCode !== ONGOING}
        >
          Step
        </button>
        <button
          style={BUTTON_STYLE}
          onClick={stepMany}
          disabled={running || resultCode !== ONGOING}
        >
          +100
        </button>
        <button style={BUTTON_STYLE} onClick={reset}>
          Reset
        </button>
        <label style={{ fontSize: '0.8rem', color: '#888' }}>
          Speed:{' '}
          <input
            type="range"
            min={1}
            max={500}
            value={stepsPerFrame}
            onChange={(e) => setStepsPerFrame(Number(e.target.value))}
            style={{ verticalAlign: 'middle', width: '120px' }}
          />
          {' '}{stepsPerFrame}/frame
        </label>
      </div>

      <div style={STATUS_STYLE}>
        {ready ? (
          <>
            <div>
              Steps: {stepCount.toLocaleString()} / 80,000 —{' '}
              {resultText(resultCode, resultWinner)}
            </div>
            <div style={{ marginTop: '0.3rem' }}>
              {warriors.map((w, i) => (
                <span key={i} style={{ marginRight: '1.5rem' }}>
                  {w.name}: {w.alive ? `alive (${w.procs} proc${w.procs !== 1 ? 's' : ''})` : 'dead'}
                </span>
              ))}
            </div>
            <div style={{ marginTop: '0.3rem', fontSize: '0.75rem', color: '#555' }}>
              Engine v{engineVersion()}
            </div>
          </>
        ) : (
          <div>Loading engine...</div>
        )}
      </div>
    </div>
  );
}
