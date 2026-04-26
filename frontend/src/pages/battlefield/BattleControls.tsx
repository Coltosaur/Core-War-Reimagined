import { useCallback } from 'react';
import {
  BUTTON_STYLE,
  CONTROLS_STYLE,
  ONGOING,
  SPEED_INPUT_STYLE,
  SPEED_LABEL_STYLE,
  SPEED_SLIDER_STYLE,
} from './styles';

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

  return (
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
      <button style={BUTTON_STYLE} onClick={stepOnce} disabled={running || resultCode !== ONGOING}>
        Step
      </button>
      <button style={BUTTON_STYLE} onClick={stepMany} disabled={running || resultCode !== ONGOING}>
        +100
      </button>
      <button style={BUTTON_STYLE} onClick={reset}>
        Reset
      </button>
      <label style={SPEED_LABEL_STYLE}>
        Speed:
        <input
          type="range"
          min={1}
          max={500}
          value={stepsPerFrame}
          onChange={handleSliderChange}
          style={SPEED_SLIDER_STYLE}
        />
        <input
          type="number"
          min={1}
          max={500}
          value={stepsPerFrame}
          onChange={handleInputChange}
          style={SPEED_INPUT_STYLE}
        />
        /frame
      </label>
    </div>
  );
}
