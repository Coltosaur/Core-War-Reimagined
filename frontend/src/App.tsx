import { useCallback, useEffect, useRef, useState } from 'react';
import init, {
  parseWarrior,
  MatchState,
  engineVersion,
  type ParsedWarrior,
} from 'core-war-engine';
import { createCoreRenderer, type CoreRenderer } from './core/coreRenderer';
import { cellAddressAtPixel, formatCellTooltip } from './core/redcodeFormat';
import { CORE_SIZE } from './core/constants';

// ─── Built-in warrior library ────────────────────────────────────────

const WARRIOR_LIST: { label: string; source: string }[] = [
  {
    label: 'Imp',
    source: `;name Imp
        MOV.I $0, $1`,
  },
  {
    label: 'Dwarf',
    source: `;name Dwarf
;author A.K. Dewdney
        ORG    start
start   ADD.AB #4, bomb
        MOV.I  bomb, @bomb
        JMP    start
bomb    DAT.F  #0, #0`,
  },
  {
    label: 'Mice-Lite',
    source: `;name Mice-Lite
        ORG    loop
counter DAT.F  #0, #3
dest    DAT.F  #0, #8
imp     MOV.I  $0, $1
loop    MOV.I  imp, <dest
        DJN.B  loop, counter
landing DAT.F  #0, #0`,
  },
  {
    label: 'Mice',
    source: `;name Mice
;author Chip Wendell
        ORG    start
ptr     DAT.F  #0, #0
start   MOV.AB #8, ptr
loop    MOV.I  @ptr, <copy
        DJN.B  loop, ptr
        SPL    @copy, #0
        ADD.AB #653, copy
        JMZ.B  start, ptr
copy    DAT.F  #0, #833`,
  },
  {
    label: 'Scanner',
    source: `;name Scanner
        ORG    loop
ptr     DAT.F  #0, #9
blank   DAT.F  #0, #0
bomb    DAT.F  #0, #99
loop    ADD.AB #1, ptr
        SEQ.I  @ptr, blank
        JMP    found
        JMP    loop
found   MOV.I  bomb, @ptr
        JMP    loop`,
  },
];

// Warrior grid colors — must match WARRIOR_COLORS in coreRenderer.ts.
// Index = owner value (0 = unowned, 1 = warrior 0, 2 = warrior 1).
const WARRIOR_HEX = ['#888888', '#e94560', '#4fc3f7', '#4caf50', '#ffab00'];

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
  position: 'relative',
  cursor: 'crosshair',
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

function resultBanner(
  code: number,
  winnerId: number,
  names: string[],
): { text: string; color: string } | null {
  switch (code) {
    case VICTORY: {
      const name = names[winnerId] ?? `Warrior ${winnerId}`;
      const color = WARRIOR_HEX[winnerId + 1] ?? '#e0e0e0';
      return { text: `${name} wins!`, color };
    }
    case TIE:
      return { text: 'Tie — step limit reached', color: '#f0c040' };
    case ALL_DEAD:
      return { text: 'All warriors eliminated — no winner', color: '#888' };
    default:
      return null;
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
  const [pick0, setPick0] = useState(0); // index into WARRIOR_LIST
  const [pick1, setPick1] = useState(1);
  const [warriors, setWarriors] = useState<
    { name: string; alive: boolean; procs: number }[]
  >([]);

  const gridRef = useRef<HTMLDivElement>(null);
  const tooltipRef = useRef<HTMLDivElement>(null);
  const hoveredAddrRef = useRef(-1);
  const rendererRef = useRef<CoreRenderer | null>(null);
  const matchRef = useRef<MatchState | null>(null);
  const warriorNamesRef = useRef<string[]>([]);
  const pick0Ref = useRef(pick0);
  const pick1Ref = useRef(pick1);
  const rafRef = useRef(0);
  const frameCountRef = useRef(0);
  const spfRef = useRef(stepsPerFrame);

  // Keep refs in sync so callbacks see the latest values without
  // needing to be recreated.
  spfRef.current = stepsPerFrame;
  pick0Ref.current = pick0;
  pick1Ref.current = pick1;

  // ── Cell tooltip (imperative — no React re-renders on mousemove) ──

  const handleGridMouseMove = useCallback(
    (e: React.MouseEvent<HTMLDivElement>) => {
      const m = matchRef.current;
      const tip = tooltipRef.current;
      if (!m || !tip) return;

      const rect = e.currentTarget.getBoundingClientRect();
      const x = e.clientX - rect.left;
      const y = e.clientY - rect.top;
      const addr = cellAddressAtPixel(x, y);

      if (addr < 0) {
        tip.style.display = 'none';
        hoveredAddrRef.current = -1;
        return;
      }

      hoveredAddrRef.current = addr;
      tip.textContent = formatCellTooltip(m, addr);
      tip.style.display = 'block';

      // Position the tooltip near the cursor, offset slightly so it
      // doesn't occlude the cell or fight the pointer.
      const tipX = Math.min(x + 12, rect.width - tip.offsetWidth - 4);
      const tipY = y > 40 ? y - 32 : y + 20;
      tip.style.left = `${tipX}px`;
      tip.style.top = `${tipY}px`;
    },
    [],
  );

  const handleGridMouseLeave = useCallback(() => {
    hoveredAddrRef.current = -1;
    const tip = tooltipRef.current;
    if (tip) tip.style.display = 'none';
  }, []);

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

    const src0 = WARRIOR_LIST[pick0Ref.current].source;
    const src1 = WARRIOR_LIST[pick1Ref.current].source;

    let w1: ParsedWarrior, w2: ParsedWarrior;
    try {
      w1 = parseWarrior(src0);
      w2 = parseWarrior(src1);
    } catch (e) {
      console.error('Parse error:', e);
      return;
    }

    match.loadWarrior(0, w1, 0);
    match.loadWarrior(1, w2, Math.floor(CORE_SIZE / 2));
    matchRef.current = match;
    warriorNamesRef.current = [
      w1.name() ?? WARRIOR_LIST[pick0Ref.current].label,
      w2.name() ?? WARRIOR_LIST[pick1Ref.current].label,
    ];

    // Show the initial core
    if (rendererRef.current) {
      rendererRef.current.update(match.coreOwnership());
    }

    setStepCount(0);
    setResultCode(ONGOING);
    setResultWinner(-1);
    setWarriors([
      { name: warriorNamesRef.current[0], alive: true, procs: 1 },
      { name: warriorNamesRef.current[1], alive: true, procs: 1 },
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
    const names = warriorNamesRef.current;
    for (let i = 0; i < m.warriorCount(); i++) {
      ws.push({
        name: names[i] ?? `Warrior ${m.warriorId(i)}`,
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
    r.update(m.coreOwnership());

    // Live-refresh the tooltip if the mouse is hovering over a cell.
    // Cost: 6 wasm array lookups + 1 textContent write — negligible.
    if (hoveredAddrRef.current >= 0 && tooltipRef.current) {
      tooltipRef.current.textContent = formatCellTooltip(m, hoveredAddrRef.current);
    }

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
    r.update(m.coreOwnership());
    syncUiState();
  }, [syncUiState]);

  const stepMany = useCallback(() => {
    const m = matchRef.current;
    const r = rendererRef.current;
    if (!m || !r || m.resultCode() !== ONGOING) return;
    m.stepN(100);
    r.update(m.coreOwnership());
    syncUiState();
  }, [syncUiState]);

  const reset = useCallback(() => {
    cancelAnimationFrame(rafRef.current);
    setRunning(false);
    loadBattle();
  }, [loadBattle]);

  // ── Warrior picker change handler ───────────────────────────────

  const handlePickChange = useCallback(
    (side: 0 | 1, idx: number) => {
      if (side === 0) setPick0(idx);
      else setPick1(idx);
      // Reset battle with the new warrior on next frame (refs are
      // updated synchronously by the render that follows setState).
      cancelAnimationFrame(rafRef.current);
      setRunning(false);
      // Use setTimeout(0) so the ref sync happens before loadBattle reads it.
      setTimeout(() => loadBattle(), 0);
    },
    [loadBattle],
  );

  // ── Render ────────────────────────────────────────────────────────

  const selectStyle: React.CSSProperties = {
    fontFamily: 'inherit',
    fontSize: '0.85rem',
    backgroundColor: '#1e1e1e',
    color: '#e0e0e0',
    border: '1px solid #444',
    borderRadius: '4px',
    padding: '0.3rem 0.5rem',
  };

  return (
    <div style={ROOT_STYLE}>
      <h1 style={{ margin: 0, fontSize: '1.5rem', letterSpacing: '0.1em' }}>
        CORE WAR
      </h1>

      {/* ── Warrior picker ─────────────────────────────────────── */}
      <div style={{ ...CONTROLS_STYLE, gap: '0.75rem' }}>
        <label style={{ color: WARRIOR_HEX[1], fontSize: '0.85rem' }}>
          Red:{' '}
          <select
            style={selectStyle}
            value={pick0}
            onChange={(e) => handlePickChange(0, Number(e.target.value))}
          >
            {WARRIOR_LIST.map((w, i) => (
              <option key={i} value={i}>
                {w.label}
              </option>
            ))}
          </select>
        </label>
        <span style={{ color: '#555' }}>vs</span>
        <label style={{ color: WARRIOR_HEX[2], fontSize: '0.85rem' }}>
          Blue:{' '}
          <select
            style={selectStyle}
            value={pick1}
            onChange={(e) => handlePickChange(1, Number(e.target.value))}
          >
            {WARRIOR_LIST.map((w, i) => (
              <option key={i} value={i}>
                {w.label}
              </option>
            ))}
          </select>
        </label>
      </div>

      {/* ── Core grid + tooltip ────────────────────────────────── */}
      <div
        ref={gridRef}
        style={GRID_CONTAINER_STYLE}
        onMouseMove={handleGridMouseMove}
        onMouseLeave={handleGridMouseLeave}
      >
        <div
          ref={tooltipRef}
          style={{
            display: 'none',
            position: 'absolute',
            pointerEvents: 'none',
            backgroundColor: 'rgba(0, 0, 0, 0.85)',
            color: '#e0e0e0',
            fontFamily: '"JetBrains Mono", "Fira Code", monospace',
            fontSize: '0.75rem',
            padding: '0.25rem 0.5rem',
            borderRadius: '3px',
            border: '1px solid #444',
            whiteSpace: 'nowrap',
            zIndex: 10,
          }}
        />
      </div>

      {/* ── Battle controls ────────────────────────────────────── */}
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
        <label style={{ fontSize: '0.8rem', color: '#888', display: 'flex', alignItems: 'center', gap: '0.4rem' }}>
          Speed:
          <input
            type="range"
            min={1}
            max={500}
            value={stepsPerFrame}
            onChange={(e) => setStepsPerFrame(Number(e.target.value))}
            style={{ verticalAlign: 'middle', width: '100px' }}
          />
          <input
            type="number"
            min={1}
            max={500}
            value={stepsPerFrame}
            onChange={(e) => {
              const v = Math.max(1, Math.min(500, Number(e.target.value) || 1));
              setStepsPerFrame(v);
            }}
            style={{
              width: '3.5rem',
              fontFamily: 'inherit',
              fontSize: '0.8rem',
              backgroundColor: '#1e1e1e',
              color: '#e0e0e0',
              border: '1px solid #444',
              borderRadius: '3px',
              padding: '0.15rem 0.3rem',
              textAlign: 'right',
            }}
          />
          /frame
        </label>
      </div>

      {/* ── Status bar ─────────────────────────────────────────── */}
      <div style={STATUS_STYLE}>
        {ready ? (
          <>
            <div>
              Steps: {stepCount.toLocaleString()} / 80,000
            </div>
            <div style={{ marginTop: '0.3rem' }}>
              {warriors.map((w, i) => (
                <span
                  key={i}
                  style={{
                    marginRight: '1.5rem',
                    color: WARRIOR_HEX[i + 1],
                  }}
                >
                  {w.name}:{' '}
                  {w.alive
                    ? `alive (${w.procs} proc${w.procs !== 1 ? 's' : ''})`
                    : 'dead'}
                </span>
              ))}
            </div>
            {(() => {
              const banner = resultBanner(
                resultCode,
                resultWinner,
                warriorNamesRef.current,
              );
              if (!banner) return null;
              return (
                <div
                  style={{
                    marginTop: '0.75rem',
                    padding: '0.5rem 1.5rem',
                    fontSize: '1.1rem',
                    fontWeight: 600,
                    letterSpacing: '0.05em',
                    color: banner.color,
                    border: `1px solid ${banner.color}44`,
                    borderRadius: '6px',
                    backgroundColor: `${banner.color}11`,
                    textShadow: `0 0 12px ${banner.color}66`,
                  }}
                >
                  {banner.text}
                </div>
              );
            })()}
            <div
              style={{
                marginTop: '0.5rem',
                fontSize: '0.75rem',
                color: '#555',
              }}
            >
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
