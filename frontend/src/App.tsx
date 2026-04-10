import { useEffect, useState } from 'react';
import init, {
  parseWarrior,
  MatchState,
  engineVersion,
} from 'core-war-engine';

const IMP_SOURCE = 'MOV.I $0, $1';

const DWARF_SOURCE = `
;name Dwarf
;author A.K. Dewdney
        ORG    start
start   ADD.AB #4, bomb
        MOV.I  bomb, @bomb
        JMP    start
bomb    DAT.F  #0, #0
        END
`;

export default function App() {
  const [status, setStatus] = useState('Loading engine...');
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    (async () => {
      try {
        await init();

        // Quick sanity check: parse and run the Imp
        const imp = parseWarrior(IMP_SOURCE);
        const match = new MatchState(8000, 80_000);
        match.loadWarrior(0, imp, 0);
        match.stepN(10);

        // Parse the Dwarf to exercise labels + metadata
        const dwarf = parseWarrior(DWARF_SOURCE);

        setStatus(
          [
            `Engine v${engineVersion()}`,
            `Imp: ${imp.instructionCount()} instr, ran ${match.steps()} steps, core size ${match.coreSize()}`,
            `Dwarf: "${dwarf.name()}" by ${dwarf.author()}, ${dwarf.instructionCount()} instr`,
          ].join(' | '),
        );
      } catch (e) {
        setError(String(e));
      }
    })();
  }, []);

  return (
    <div style={{ fontFamily: 'monospace', padding: '2rem' }}>
      <h1>Core War</h1>
      {error ? (
        <p style={{ color: 'red' }}>Error: {error}</p>
      ) : (
        <p>{status}</p>
      )}
    </div>
  );
}
