import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useSearchParams } from 'react-router-dom';
import init, { parseWarrior, MatchState, engineVersion, type ParsedWarrior } from 'core-war-engine';
import { createCoreRenderer, type CoreRenderer } from '../core/coreRenderer';
import { cellAddressAtPixel, formatCellTooltip } from '../core/redcodeFormat';
import { CORE_SIZE } from '../core/constants';
import { useWarriorLibrary, type Warrior } from '../warriors/library';

const WARRIOR_HEX = ['#888888', '#e94560', '#4fc3f7', '#4caf50', '#ffab00'];

const ROOT_STYLE: React.CSSProperties = {
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

function pickInitial(library: Warrior[], queryId: string | null, fallbackIdx: number): string {
  if (queryId && library.some((w) => w.id === queryId)) return queryId;
  return library[fallbackIdx]?.id ?? library[0]?.id ?? '';
}

export default function BattlefieldPage() {
  const library = useWarriorLibrary();
  const [searchParams, setSearchParams] = useSearchParams();

  const [ready, setReady] = useState(false);
  const [running, setRunning] = useState(false);
  const [stepCount, setStepCount] = useState(0);
  const [resultCode, setResultCode] = useState(ONGOING);
  const [resultWinner, setResultWinner] = useState(-1);
  const [stepsPerFrame, setStepsPerFrame] = useState(50);
  const [redId, setRedId] = useState(() => pickInitial(library, searchParams.get('red'), 0));
  const [blueId, setBlueId] = useState(() => pickInitial(library, searchParams.get('blue'), 1));
  const [warriors, setWarriors] = useState<{ name: string; alive: boolean; procs: number }[]>([]);
  const [parseError, setParseError] = useState<string | null>(null);

  const gridRef = useRef<HTMLDivElement>(null);
  const tooltipRef = useRef<HTMLDivElement>(null);
  const hoveredAddrRef = useRef(-1);
  const rendererRef = useRef<CoreRenderer | null>(null);
  const matchRef = useRef<MatchState | null>(null);
  const warriorNamesRef = useRef<string[]>([]);
  const rafRef = useRef(0);
  const frameCountRef = useRef(0);
  const spfRef = useRef(stepsPerFrame);

  // Keep the URL in sync with current selection so handoff from the builder is linkable.
  useEffect(() => {
    const next = new URLSearchParams(searchParams);
    next.set('red', redId);
    next.set('blue', blueId);
    if (next.toString() !== searchParams.toString()) {
      setSearchParams(next, { replace: true });
    }
  }, [redId, blueId, searchParams, setSearchParams]);

  const libraryById = useMemo(() => {
    const map = new Map<string, Warrior>();
    library.forEach((w) => map.set(w.id, w));
    return map;
  }, [library]);

  const handleGridMouseMove = useCallback((e: React.MouseEvent<HTMLDivElement>) => {
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

    const tipX = Math.min(x + 12, rect.width - tip.offsetWidth - 4);
    const tipY = y > 40 ? y - 32 : y + 20;
    tip.style.left = `${tipX}px`;
    tip.style.top = `${tipY}px`;
  }, []);

  const handleGridMouseLeave = useCallback(() => {
    hoveredAddrRef.current = -1;
    const tip = tooltipRef.current;
    if (tip) tip.style.display = 'none';
  }, []);

  const readyRef = useRef(false);

  useEffect(() => {
    if (!gridRef.current) return;
    const renderer = createCoreRenderer(gridRef.current);
    rendererRef.current = renderer;
    return () => {
      renderer.destroy();
      rendererRef.current = null;
    };
  }, []);

  const loadBattle = useCallback(
    (currentRedId: string, currentBlueId: string) => {
      if (!readyRef.current) return;

      cancelAnimationFrame(rafRef.current);
      setRunning(false);

      const red = libraryById.get(currentRedId);
      const blue = libraryById.get(currentBlueId);
      if (!red || !blue) {
        setParseError('Selected warrior not found in library.');
        return;
      }

      let w1: ParsedWarrior, w2: ParsedWarrior;
      try {
        w1 = parseWarrior(red.source);
        w2 = parseWarrior(blue.source);
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        setParseError(`Parse error: ${msg}`);
        return;
      }
      setParseError(null);

      const match = new MatchState(CORE_SIZE, 80_000);
      match.loadWarrior(0, w1, 0);
      match.loadWarrior(1, w2, Math.floor(CORE_SIZE / 2));
      matchRef.current = match;
      warriorNamesRef.current = [w1.name() ?? red.label, w2.name() ?? blue.label];

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
    },
    [libraryById],
  );

  useEffect(() => {
    let cancelled = false;
    init().then(() => {
      if (cancelled) return;
      readyRef.current = true;
      setReady(true);
      loadBattle(redId, blueId);
    });
    return () => {
      cancelled = true;
    };
    // Only run once on mount — warrior changes are handled by event handlers
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

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

  const tickRef = useRef<() => void>(() => {});
  useEffect(() => {
    tickRef.current = () => {
      const m = matchRef.current;
      const r = rendererRef.current;
      if (!m || !r) return;

      m.stepN(spfRef.current);
      r.update(m.coreOwnership());

      if (hoveredAddrRef.current >= 0 && tooltipRef.current) {
        tooltipRef.current.textContent = formatCellTooltip(m, hoveredAddrRef.current);
      }

      frameCountRef.current++;
      if (frameCountRef.current % 6 === 0) {
        syncUiState();
      }

      if (m.resultCode() === ONGOING) {
        rafRef.current = requestAnimationFrame(() => tickRef.current());
      } else {
        syncUiState();
        setRunning(false);
      }
    };
  }, [syncUiState]);

  const play = useCallback(() => {
    if (!matchRef.current) return;
    if (matchRef.current.resultCode() !== ONGOING) return;
    setRunning(true);
    frameCountRef.current = 0;
    rafRef.current = requestAnimationFrame(() => tickRef.current());
  }, []);

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
    loadBattle(redId, blueId);
  }, [loadBattle, redId, blueId]);

  const handlePickChange = useCallback(
    (side: 0 | 1, id: string) => {
      const newRed = side === 0 ? id : redId;
      const newBlue = side === 1 ? id : blueId;
      if (side === 0) setRedId(id);
      else setBlueId(id);
      cancelAnimationFrame(rafRef.current);
      setRunning(false);
      loadBattle(newRed, newBlue);
    },
    [loadBattle, redId, blueId],
  );

  const selectStyle: React.CSSProperties = {
    fontFamily: 'inherit',
    fontSize: '0.85rem',
    backgroundColor: '#1e1e1e',
    color: '#e0e0e0',
    border: '1px solid #444',
    borderRadius: '4px',
    padding: '0.3rem 0.5rem',
  };

  const presets = library.filter((w) => w.isPreset);
  const userWarriors = library.filter((w) => !w.isPreset);

  return (
    <div style={ROOT_STYLE}>
      <h1 style={{ margin: 0, fontSize: '1.5rem', letterSpacing: '0.1em' }}>CORE WAR</h1>

      <div style={{ ...CONTROLS_STYLE, gap: '0.75rem' }}>
        <label style={{ color: WARRIOR_HEX[1], fontSize: '0.85rem' }}>
          Red:{' '}
          <select
            style={selectStyle}
            value={redId}
            onChange={(e) => handlePickChange(0, e.target.value)}
          >
            <optgroup label="Classic">
              {presets.map((w) => (
                <option key={w.id} value={w.id}>
                  {w.label}
                </option>
              ))}
            </optgroup>
            {userWarriors.length > 0 && (
              <optgroup label="My Warriors">
                {userWarriors.map((w) => (
                  <option key={w.id} value={w.id}>
                    {w.label}
                  </option>
                ))}
              </optgroup>
            )}
          </select>
        </label>
        <span style={{ color: '#555' }}>vs</span>
        <label style={{ color: WARRIOR_HEX[2], fontSize: '0.85rem' }}>
          Blue:{' '}
          <select
            style={selectStyle}
            value={blueId}
            onChange={(e) => handlePickChange(1, e.target.value)}
          >
            <optgroup label="Classic">
              {presets.map((w) => (
                <option key={w.id} value={w.id}>
                  {w.label}
                </option>
              ))}
            </optgroup>
            {userWarriors.length > 0 && (
              <optgroup label="My Warriors">
                {userWarriors.map((w) => (
                  <option key={w.id} value={w.id}>
                    {w.label}
                  </option>
                ))}
              </optgroup>
            )}
          </select>
        </label>
      </div>

      {parseError && (
        <div
          style={{
            padding: '0.5rem 1rem',
            color: '#e94560',
            border: '1px solid #e9456066',
            borderRadius: '4px',
            backgroundColor: '#e9456011',
            fontSize: '0.85rem',
          }}
        >
          {parseError}
        </div>
      )}

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
        <label
          style={{
            fontSize: '0.8rem',
            color: '#888',
            display: 'flex',
            alignItems: 'center',
            gap: '0.4rem',
          }}
        >
          Speed:
          <input
            type="range"
            min={1}
            max={500}
            value={stepsPerFrame}
            onChange={(e) => {
              const v = Number(e.target.value);
              setStepsPerFrame(v);
              spfRef.current = v;
            }}
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
              spfRef.current = v;
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

      <div style={STATUS_STYLE}>
        {ready ? (
          <>
            <div>Steps: {stepCount.toLocaleString()} / 80,000</div>
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
                  {w.alive ? `alive (${w.procs} proc${w.procs !== 1 ? 's' : ''})` : 'dead'}
                </span>
              ))}
            </div>
            {(() => {
              const banner = resultBanner(
                resultCode,
                resultWinner,
                warriors.map((w) => w.name),
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
