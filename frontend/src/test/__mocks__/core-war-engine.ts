import { vi } from 'vitest';

// Behavior-faithful `parseWarrior` mock. Prior to the #85 sweep, the mock
// returned a successful parse for ANY input, including `''` — which is why
// the "Builder shows EmptyWarrior on fresh load" bug slipped through the
// hook-under-test's own coverage: every test file that used the shared mock
// saw the buggy `runParse('')` path as "ok". Now the mock throws
// EmptyWarrior on empty input the same way the real engine does, so any
// component that accidentally calls `parseWarrior('')` will show up as a
// failing test rather than a silent lie.
//
// If a test wants to override this behavior (e.g. simulate a parse error),
// it can do so via `vi.mocked(parseWarrior).mockImplementation(...)` in a
// `beforeEach`, exactly like useBuilder.test.tsx does today.
const EMPTY_WARRIOR_MESSAGE = 'warrior source has no instructions';

export const parseWarrior = vi.fn((source: string) => {
  if (!source || !source.trim()) throw new Error(EMPTY_WARRIOR_MESSAGE);
  return {
    name: () => 'Test Warrior',
    author: () => 'Test Author',
    instructionCount: () => 1,
    startOffset: () => 0,
  };
});

export const engineVersion = vi.fn(() => '0.1.0-mock');

export class MatchState {
  private _coreSize: number;
  private _step = 0;

  constructor(coreSize: number, _maxSteps: number) {
    this._coreSize = coreSize;
    void _maxSteps;
  }

  loadWarrior(): void {}
  step(): void {
    this._step++;
  }
  stepN(n: number): void {
    this._step += n;
  }
  resultCode(): number {
    return 0;
  }
  resultWinnerId(): number {
    return -1;
  }
  coreOpcodes(): Uint8Array {
    return new Uint8Array(this._coreSize);
  }
  coreSnapshot(): Uint32Array {
    return new Uint32Array(this._coreSize * 2);
  }
  cellOpcode(): number {
    return 0;
  }
  cellModifier(): number {
    return 0;
  }
  cellAMode(): number {
    return 0;
  }
  cellAValue(): number {
    return 0;
  }
  cellBMode(): number {
    return 0;
  }
  cellBValue(): number {
    return 0;
  }
  warriorCount(): number {
    return 2;
  }
  warriorIsAlive(): boolean {
    return true;
  }
  warriorProcessCount(): number {
    return 1;
  }
  warriorProcessPcs(): Uint32Array {
    return new Uint32Array([0]);
  }
}

export type ParsedWarrior = ReturnType<typeof parseWarrior>;

const init = vi.fn(() => Promise.resolve());
export default init;
