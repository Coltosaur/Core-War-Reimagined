import { useCallback } from 'react';
import { ONGOING } from './battleResult';
import controls from '../../components/controls.module.css';

type Props = {
  running: boolean;
  resultCode: number;
  stepsPerFrame: number;
  setStepsPerFrame: (v: number) => void;
  spfRef: React.MutableRefObject<number>;
  play: () => void;
  pause: () => void;
  stepOnce: () => void;
  stepMany: () => void;
  reset: () => void;
};

export default function BattleControls({
  running,
  resultCode,
  stepsPerFrame,
  setStepsPerFrame,
  spfRef,
  play,
  pause,
  stepOnce,
  stepMany,
  reset,
}: Props) {
  const handleSliderChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const v = Number(e.target.value);
      setStepsPerFrame(v);
      spfRef.current = v;
    },
    [setStepsPerFrame, spfRef],
  );

  const handleInputChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const v = Math.max(1, Math.min(500, Number(e.target.value) || 1));
      setStepsPerFrame(v);
      spfRef.current = v;
    },
    [setStepsPerFrame, spfRef],
  );

  const stepDisabled = running || resultCode !== ONGOING;

  return (
    <div className={controls.row}>
      {!running ? (
        <button type="button" className={`${controls.button} ${controls.primary}`} onClick={play}>
          Play
        </button>
      ) : (
        <button type="button" className={`${controls.button} ${controls.primary}`} onClick={pause}>
          Pause
        </button>
      )}
      <button type="button" className={controls.button} onClick={stepOnce} disabled={stepDisabled}>
        Step
      </button>
      <button type="button" className={controls.button} onClick={stepMany} disabled={stepDisabled}>
        +100
      </button>
      <button type="button" className={controls.button} onClick={reset}>
        Reset
      </button>
      <label className={controls.speed}>
        Speed
        <input
          type="range"
          min={1}
          max={500}
          value={stepsPerFrame}
          onChange={handleSliderChange}
          className={controls.slider}
          aria-label="Steps per frame"
        />
        <input
          type="number"
          min={1}
          max={500}
          value={stepsPerFrame}
          onChange={handleInputChange}
          className={controls.number}
          aria-label="Steps per frame (exact)"
        />
        /frame
      </label>
    </div>
  );
}
