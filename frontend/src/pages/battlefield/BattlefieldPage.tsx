import { useBattle } from './useBattle';
import WarriorSelector from './WarriorSelector';
import BattleControls from './BattleControls';
import BattleStatus from './BattleStatus';
import InspectorPanel from './InspectorPanel';
import alertStyles from '../../components/alert.module.css';
import gridStyles from '../../core/CoreGrid.module.css';
import styles from './BattlefieldPage.module.css';

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
    <div className={styles.page}>
      <div className={styles.arena}>
        <h1 className={styles.title}>Battlefield</h1>

        <WarriorSelector
          redId={redId}
          blueId={blueId}
          presets={presets}
          userWarriors={userWarriors}
          onPickChange={handlePickChange}
        />

        {parseError && (
          <div role="alert" className={alertStyles.error}>
            {parseError}
          </div>
        )}

        <div className={styles.gridDock}>
          <div
            ref={gridRef}
            className={gridStyles.grid}
            onMouseMove={handleGridMouseMove}
            onMouseLeave={handleGridMouseLeave}
            onClick={handleGridClick}
          >
            <div ref={tooltipRef} className={gridStyles.tooltip} />
          </div>
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
        className={styles.inspector}
      />
    </div>
  );
}
