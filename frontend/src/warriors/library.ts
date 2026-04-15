import { useSyncExternalStore } from 'react';

export type Warrior = {
  id: string;
  label: string;
  source: string;
  isPreset: boolean;
};

const PRESETS: ReadonlyArray<Warrior> = [
  {
    id: 'preset:imp',
    label: 'Imp',
    isPreset: true,
    source: `;name Imp
;author A.K. Dewdney
; The simplest possible warrior: one instruction that copies itself
; one cell forward, then the PC advances to that copy. The imp walks
; through the core at one cell per step, leaving a trail of MOV.I
; instructions behind it.
;
; MOV.I $0, $1
;   MOV    — move (copy) the source operand into the destination.
;   .I     — the "I" modifier copies the whole instruction, not a field.
;   $0     — A operand: direct, offset 0 (this cell itself).
;   $1     — B operand: direct, offset 1 (the next cell).
;
; After it runs: cell N+1 now contains the same MOV.I, and the PC
; moves on to cell N+1 — which is our MOV.I — and does it all again.
        MOV.I $0, $1`,
  },
  {
    id: 'preset:dwarf',
    label: 'Dwarf',
    isPreset: true,
    source: `;name Dwarf
;author A.K. Dewdney
; The canonical bomber. Sits in place and drops DAT bombs at a fixed
; stride through the core. If the stride is coprime with the core size
; every cell gets hit eventually — so anything not defending itself
; will be killed.
        ORG    start          ; start execution at the "start" label

start   ADD.AB #4, bomb       ; add 4 to bomb's B-field — advance the target
        MOV.I  bomb, @bomb    ; copy "bomb" to where bomb's B-field points
                              ;   @bomb = B-indirect: address computed
                              ;   by following bomb's B-field
        JMP    start          ; loop forever

bomb    DAT.F  #0, #0         ; the payload: a DAT kills any process that
                              ; tries to execute it. B-field gets rewritten
                              ; each iteration to aim at a new target.`,
  },
  {
    id: 'preset:mice-lite',
    label: 'Mice-Lite',
    isPreset: true,
    source: `;name Mice-Lite
; A stripped-down "paper" (replicator) strategy. Copies an imp into a
; nearby landing zone and starts it running there. Papers survive by
; outnumbering attackers — every copy is another thread to kill.
        ORG    loop

counter DAT.F  #0, #3         ; how many imp cells to copy (3)
dest    DAT.F  #0, #8         ; offset to landing zone (8 cells ahead)
imp     MOV.I  $0, $1         ; the imp we're going to replicate

loop    MOV.I  imp, <dest     ; copy "imp" to the cell pointed at by dest,
                              ;   <dest = B-predecrement: decrement dest's
                              ;   B-field first, then use it as the address
        DJN.B  loop, counter  ; decrement counter's B-field, jump back
                              ; to loop until counter hits zero
landing DAT.F  #0, #0         ; this is where dest starts pointing`,
  },
  {
    id: 'preset:mice',
    label: 'Mice',
    isPreset: true,
    source: `;name Mice
;author Chip Wendell
; The classic replicator. Copies itself to a distant cell, splits off a
; new process to run the copy, then loops to do it again in yet another
; location. Each spawned mouse does the same — the warrior multiplies
; exponentially until process limit or step limit hits.
        ORG    start

ptr     DAT.F  #0, #0         ; points at the next source cell to copy
start   MOV.AB #8, ptr        ; set ptr to 8 — copy 8 cells (the warrior
                              ; body) before spawning
loop    MOV.I  @ptr, <copy    ; copy cell at ptr → cell before <copy's
                              ; destination (predecrement moves the target
                              ; back one before writing)
        DJN.B  loop, ptr      ; loop until ptr's B-field hits zero
        SPL    @copy, #0      ; spawn a NEW process starting at the
                              ; address in copy's B-field — now two
                              ; processes run this code
        ADD.AB #653, copy     ; shift the destination 653 cells ahead so
                              ; the next cycle lands somewhere new
        JMZ.B  start, ptr     ; loop back and do it again

copy    DAT.F  #0, #833       ; destination pointer for the next copy`,
  },
  {
    id: 'preset:scanner',
    label: 'Scanner',
    isPreset: true,
    source: `;name Scanner
; A simple linear scanner. Walks the core looking for any occupied
; cell. When it finds one, it drops a DAT bomb there — much more
; efficient than Dwarf's blind bombing because it doesn't waste time
; hitting empty cells.
        ORG    loop

ptr     DAT.F  #0, #9         ; current scan position (relative offset)
blank   DAT.F  #0, #0         ; what an empty cell looks like
bomb    DAT.F  #0, #99        ; the DAT we'll drop on anything non-empty

loop    ADD.AB #1, ptr        ; advance the scan pointer by 1
        SEQ.I  @ptr, blank    ; skip next instruction IF cell at ptr
                              ; equals blank (i.e. is empty)
        JMP    found          ; → reached only if cell is NOT blank
        JMP    loop           ; cell was blank, keep scanning
found   MOV.I  bomb, @ptr     ; drop the bomb on the non-empty cell
        JMP    loop           ; back to scanning`,
  },
];

const STORAGE_KEY = 'corewar.warriors.v1';

type StoredWarrior = { id: string; label: string; source: string };

function loadUserWarriors(): Warrior[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw) as StoredWarrior[];
    if (!Array.isArray(parsed)) return [];
    return parsed.map((w) => ({ ...w, isPreset: false }));
  } catch {
    return [];
  }
}

function saveUserWarriors(list: Warrior[]): void {
  const serialized: StoredWarrior[] = list
    .filter((w) => !w.isPreset)
    .map(({ id, label, source }) => ({ id, label, source }));
  localStorage.setItem(STORAGE_KEY, JSON.stringify(serialized));
}

let userWarriors: Warrior[] = loadUserWarriors();
const listeners = new Set<() => void>();

function emit(): void {
  saveUserWarriors(userWarriors);
  snapshot = computeSnapshot();
  listeners.forEach((l) => l());
}

let snapshot: Warrior[] = computeSnapshot();
function computeSnapshot(): Warrior[] {
  return [...PRESETS, ...userWarriors];
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

function getSnapshot(): Warrior[] {
  return snapshot;
}

export function useWarriorLibrary(): Warrior[] {
  return useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
}

export function listWarriors(): Warrior[] {
  return snapshot;
}

export function getWarrior(id: string): Warrior | undefined {
  return snapshot.find((w) => w.id === id);
}

function randomId(): string {
  return `user:${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
}

export function createUserWarrior(label: string, source: string): Warrior {
  const w: Warrior = { id: randomId(), label, source, isPreset: false };
  userWarriors = [...userWarriors, w];
  emit();
  return w;
}

export function updateUserWarrior(
  id: string,
  patch: { label?: string; source?: string },
): void {
  userWarriors = userWarriors.map((w) =>
    w.id === id && !w.isPreset ? { ...w, ...patch } : w,
  );
  emit();
}

export function deleteUserWarrior(id: string): void {
  userWarriors = userWarriors.filter((w) => w.id !== id);
  emit();
}

export function duplicateWarrior(sourceId: string): Warrior | undefined {
  const src = getWarrior(sourceId);
  if (!src) return undefined;
  const label = src.isPreset ? `${src.label} (copy)` : `${src.label} (copy)`;
  return createUserWarrior(label, src.source);
}
