import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useSearchParams } from 'react-router-dom';
import init, { parseWarrior, MatchState, type ParsedWarrior } from 'core-war-engine';
import { createCoreRenderer, type CoreRenderer } from '../../core/coreRenderer';
import { cellAddressAtPixel, formatCellTooltip } from '../../core/redcodeFormat';
import { CORE_SIZE } from '../../core/constants';
import { useWarriorLibrary, type Warrior } from '../../warriors/library';
import { ONGOING } from './styles';

function pickInitial(library: Warrior[], queryId: string | null, fallbackIdx: number): string {
  if (queryId && library.some((w) => w.id === queryId)) return queryId;
  return library[fallbackIdx]?.id ?? library[0]?.id ?? '';
}

export function useBattle() {
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
  const readyRef = useRef(false);

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

  const presets = useMemo(() => library.filter((w) => w.isPreset), [library]);
  const userWarriors = useMemo(() => library.filter((w) => !w.isPreset), [library]);

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

  return {
    ready,
    running,
    stepCount,
    resultCode,
    resultWinner,
    stepsPerFrame,
    setStepsPerFrame,
    spfRef,
    redId,
    blueId,
    warriors,
    parseError,
    presets,
    userWarriors,
    gridRef,
    tooltipRef,
    play,
    pause,
    stepOnce,
    stepMany,
    reset,
    handlePickChange,
    handleGridMouseMove,
    handleGridMouseLeave,
  };
}
