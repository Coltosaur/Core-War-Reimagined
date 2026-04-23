import { vi } from 'vitest';

export const parseWarrior = vi.fn(() => ({
  name: () => 'Test Warrior',
  author: () => 'Test Author',
  instructionCount: () => 1,
  startOffset: () => 0,
}));

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
