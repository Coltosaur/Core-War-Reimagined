import { useBattle } from './useBattle';
import WarriorSelector from './WarriorSelector';
import BattleControls from './BattleControls';
import BattleStatus from './BattleStatus';
import {
  GRID_CONTAINER_STYLE,
  PARSE_ERROR_STYLE,
  ROOT_STYLE,
  TITLE_STYLE,
  TOOLTIP_STYLE,
} from './styles';

export default function BattlefieldPage() {
  const {
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
  } = useBattle();

  return (
    <div style={ROOT_STYLE}>
      <h1 style={TITLE_STYLE}>CORE WAR</h1>

      <WarriorSelector
        redId={redId}
        blueId={blueId}
        presets={presets}
        userWarriors={userWarriors}
        onPickChange={handlePickChange}
      />

      {parseError && <div style={PARSE_ERROR_STYLE}>{parseError}</div>}

      <div
        ref={gridRef}
        style={GRID_CONTAINER_STYLE}
        onMouseMove={handleGridMouseMove}
        onMouseLeave={handleGridMouseLeave}
      >
        <div ref={tooltipRef} style={TOOLTIP_STYLE} />
      </div>

      <BattleControls
        running={running}
        resultCode={resultCode}
        stepsPerFrame={stepsPerFrame}
        setStepsPerFrame={setStepsPerFrame}
        spfRef={spfRef}
        play={play}
        pause={pause}
        stepOnce={stepOnce}
        stepMany={stepMany}
        reset={reset}
      />

      <BattleStatus
        ready={ready}
        stepCount={stepCount}
        warriors={warriors}
        resultCode={resultCode}
        resultWinner={resultWinner}
      />
    </div>
  );
}
