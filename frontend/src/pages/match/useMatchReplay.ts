import { useCallback, useEffect, useRef, useState } from 'react';
import init, { parseWarrior, MatchState } from 'core-war-engine';
import { createCoreRenderer, type CoreRenderer } from '../../core/coreRenderer';

export type MatchStartPayload = {
  match_id: string;
  red_username: string;
  blue_username: string;
  red_warrior_source: string;
  blue_warrior_source: string;
  core_size: number;
  max_steps: number;
  red_start: number;
  blue_start: number;
  steps_taken: number;
  result: string;
  playback_start_time_ms: number;
  steps_per_sec: number;
};

export type WarriorState = {
  name: string;
  alive: boolean;
  procs: number;
};

/**
 * Pure pacing function — returns the local replay's target step at a given
 * wall-clock instant given the server's match:start payload. Clamped to
 * [0, steps_taken] to tolerate clock skew before the start time and to stop
 * advancing once the canonical match length is reached.
 */
export function computeTargetStep(
  nowMs: number,
  playbackStartTimeMs: number,
  stepsPerSec: number,
  stepsTaken: number,
): number {
  const elapsedMs = nowMs - playbackStartTimeMs;
  if (elapsedMs <= 0) return 0;
  const target = Math.floor((elapsedMs * stepsPerSec) / 1000);
  return target < stepsTaken ? target : stepsTaken;
}

export function useMatchReplay(payload: MatchStartPayload | null) {
  const [ready, setReady] = useState(false);
  const [currentStep, setCurrentStep] = useState(0);
  const [warriors, setWarriors] = useState<WarriorState[]>([]);
  const [finished, setFinished] = useState(false);
  const [parseError, setParseError] = useState<string | null>(null);

  const gridRef = useRef<HTMLDivElement>(null);
  const rendererRef = useRef<CoreRenderer | null>(null);
  const matchRef = useRef<MatchState | null>(null);
  const warriorNamesRef = useRef<string[]>([]);
  const rafRef = useRef(0);
  const frameCountRef = useRef(0);

  // Mount the PixiJS grid renderer once.
  useEffect(() => {
    if (!gridRef.current) return;
    const renderer = createCoreRenderer(gridRef.current);
    rendererRef.current = renderer;
    return () => {
      renderer.destroy();
      rendererRef.current = null;
    };
  }, []);

  const syncWarriors = useCallback(() => {
    const m = matchRef.current;
    if (!m) return;
    const names = warriorNamesRef.current;
    const next: WarriorState[] = [];
    for (let i = 0; i < m.warriorCount(); i++) {
      next.push({
        name: names[i] ?? `Warrior ${i}`,
        alive: m.warriorIsAlive(i),
        procs: m.warriorProcessCount(i),
      });
    }
    setWarriors(next);
  }, []);

  // Initialize the wasm engine and load the warriors once the payload arrives.
  useEffect(() => {
    if (!payload) return;
    let cancelled = false;

    init().then(() => {
      if (cancelled) return;
      try {
        const w1 = parseWarrior(payload.red_warrior_source);
        const w2 = parseWarrior(payload.blue_warrior_source);
        const match = new MatchState(payload.core_size, payload.max_steps);
        match.loadWarrior(0, w1, payload.red_start);
        match.loadWarrior(1, w2, payload.blue_start);
        matchRef.current = match;
        warriorNamesRef.current = [
          w1.name() ?? payload.red_username,
          w2.name() ?? payload.blue_username,
        ];
        if (rendererRef.current) {
          rendererRef.current.update(match.coreOwnership());
        }
        syncWarriors();
        setReady(true);
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        setParseError(`Engine init failed: ${msg}`);
      }
    });

    return () => {
      cancelled = true;
    };
  }, [payload, syncWarriors]);

  // Drive the paced replay loop.
  useEffect(() => {
    if (!ready || !payload) return;

    function tick() {
      const m = matchRef.current;
      const r = rendererRef.current;
      if (!m || !r || !payload) return;

      const target = computeTargetStep(
        Date.now(),
        payload.playback_start_time_ms,
        payload.steps_per_sec,
        payload.steps_taken,
      );

      const cur = m.steps();
      const delta = target - cur;
      if (delta > 0) {
        m.stepN(delta);
        r.update(m.coreOwnership());
      }

      // Throttle React state updates — once every ~6 frames is plenty for
      // the counter/process-count UI; the grid itself updates every frame.
      frameCountRef.current++;
      if (frameCountRef.current % 6 === 0) {
        setCurrentStep(m.steps());
        syncWarriors();
      }

      if (target >= payload.steps_taken) {
        setCurrentStep(m.steps());
        syncWarriors();
        setFinished(true);
        return;
      }

      rafRef.current = requestAnimationFrame(tick);
    }

    rafRef.current = requestAnimationFrame(tick);
    return () => {
      cancelAnimationFrame(rafRef.current);
    };
  }, [ready, payload, syncWarriors]);

  return {
    ready,
    currentStep,
    totalSteps: payload?.steps_taken ?? 0,
    warriors,
    finished,
    parseError,
    gridRef,
  };
}
