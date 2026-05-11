import { useBattle } from './useBattle';
import WarriorSelector from './WarriorSelector';
import BattleControls from './BattleControls';
import BattleStatus from './BattleStatus';
import InspectorPanel from './InspectorPanel';
import { GRID_CONTAINER_STYLE, PARSE_ERROR_STYLE, TITLE_STYLE, TOOLTIP_STYLE } from './styles';

const PAGE_STYLE: React.CSSProperties = {
  display: 'flex',
  height: '100vh',
  minHeight: 0,
};

const MAIN_STYLE: React.CSSProperties = {
  flex: 1,
  minWidth: 0,
  display: 'flex',
  flexDirection: 'column',
  alignItems: 'center',
  padding: '1.5rem',
  gap: '1rem',
  overflow: 'auto',
};

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
    processes,
    cellInfo,
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
    handleGridClick,
    selectCell,
    clearCell,
  } = useBattle();

  return (
    <div style={PAGE_STYLE}>
      <div style={MAIN_STYLE}>
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
          onClick={handleGridClick}
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

      <InspectorPanel
        cellInfo={cellInfo}
        warriors={processes}
        onCellSelect={selectCell}
        onClearCell={clearCell}
      />
    </div>
  );
}
