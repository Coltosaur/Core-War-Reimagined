import { useSyncExternalStore } from 'react';
import * as warriorsApi from '../api/warriors';

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

// Two localStorage keys, one per id namespace:
//   - STORAGE_KEY holds local `user:*` drafts (anonymous, never synced).
//   - SERVER_CACHE_KEY holds a stale-while-revalidate copy of the logged-in
//     user's `server:*` warriors, tagged with the owning user_id so a
//     different account on the same browser never sees someone else's list.
// Keeping them separate means a sync can replace the server namespace
// wholesale without touching drafts, and logout can drop the cache without
// touching drafts either.
const STORAGE_KEY = 'corewar.warriors.v1';
const SERVER_CACHE_KEY = 'corewar.serverWarriors.v1';

type StoredWarrior = { id: string; label: string; source: string };
type ServerCache = { userId: string; warriors: StoredWarrior[] };

function isServerId(id: string): boolean {
  return id.startsWith('server:');
}

function readStoredList(raw: unknown): Warrior[] {
  if (!Array.isArray(raw)) return [];
  return raw
    .filter(
      (w): w is StoredWarrior =>
        !!w &&
        typeof w.id === 'string' &&
        typeof w.label === 'string' &&
        typeof w.source === 'string',
    )
    .map(({ id, label, source }) => ({ id, label, source, isPreset: false }));
}

function loadLocalDrafts(): Warrior[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return [];
    // Older builds never wrote server:* here, but filter defensively so the
    // namespaces can't bleed into each other.
    return readStoredList(JSON.parse(raw)).filter((w) => !isServerId(w.id));
  } catch {
    return [];
  }
}

function loadServerCache(): { userId: string | null; warriors: Warrior[] } {
  try {
    const raw = localStorage.getItem(SERVER_CACHE_KEY);
    if (!raw) return { userId: null, warriors: [] };
    const parsed = JSON.parse(raw) as Partial<ServerCache>;
    if (typeof parsed?.userId !== 'string') return { userId: null, warriors: [] };
    const warriors = readStoredList(parsed.warriors).filter((w) => isServerId(w.id));
    return { userId: parsed.userId, warriors };
  } catch {
    return { userId: null, warriors: [] };
  }
}

function toStored(list: Warrior[]): StoredWarrior[] {
  return list.map(({ id, label, source }) => ({ id, label, source }));
}

function persist(): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(toStored(localDrafts)));
    if (serverOwner) {
      const cache: ServerCache = { userId: serverOwner, warriors: toStored(serverWarriors) };
      localStorage.setItem(SERVER_CACHE_KEY, JSON.stringify(cache));
    } else {
      localStorage.removeItem(SERVER_CACHE_KEY);
    }
  } catch {
    // Quota exceeded / storage disabled — in-memory state is still correct.
  }
}

let localDrafts: Warrior[] = loadLocalDrafts();
const initialCache = loadServerCache();
let serverWarriors: Warrior[] = initialCache.warriors;
let serverOwner: string | null = initialCache.userId;
// Bumped by every server-namespace mutation. A sync whose fetch started
// before a create/update/delete landed would otherwise overwrite that
// mutation with a pre-mutation list, so it discards that response and
// re-fetches (see syncFromServer).
let serverVersion = 0;
// A sync re-fetches at most this many times when mutations keep landing
// mid-fetch; after that the list is left as-is until the next sync.
const MAX_SYNC_ATTEMPTS = 3;

const listeners = new Set<() => void>();

function emit(): void {
  persist();
  snapshot = computeSnapshot();
  listeners.forEach((l) => l());
}

let snapshot: Warrior[] = computeSnapshot();
function computeSnapshot(): Warrior[] {
  return [...PRESETS, ...localDrafts, ...serverWarriors];
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
  localDrafts = [...localDrafts, w];
  emit();
  return w;
}

export function updateUserWarrior(id: string, patch: { label?: string; source?: string }): void {
  localDrafts = localDrafts.map((w) => (w.id === id ? { ...w, ...patch } : w));
  emit();
}

export function deleteUserWarrior(id: string): void {
  localDrafts = localDrafts.filter((w) => w.id !== id);
  emit();
}

export function duplicateWarrior(sourceId: string): Warrior | undefined {
  const src = getWarrior(sourceId);
  if (!src) return undefined;
  const label = `${src.label} (copy)`;
  return createUserWarrior(label, src.source);
}

/**
 * Point the server cache at `userId`. If the cached list belongs to a
 * different account (or there is no owner yet), it is dropped immediately so
 * one user's warriors never render under another user's session. Call before
 * `syncFromServer()` once the authenticated user is known.
 */
export function setServerOwner(userId: string): void {
  if (serverOwner === userId) return;
  serverOwner = userId;
  serverWarriors = [];
  serverVersion++;
  emit();
}

/**
 * Drop the cached server warriors (logout / forced logout). Local `user:*`
 * drafts are untouched.
 */
export function clearServerWarriors(): void {
  if (serverOwner === null && serverWarriors.length === 0) return;
  serverOwner = null;
  serverWarriors = [];
  serverVersion++;
  emit();
}

/**
 * Revalidate the server namespace. Replaces only `server:*` entries — local
 * drafts are preserved — and persists the result so it survives a reload on
 * any page. Entries deleted on another device disappear here (stale-entry
 * reconciliation). Network failures leave the cached list in place.
 *
 * If a create/update/delete lands while the list request is in flight, the
 * response predates it and is discarded; the list is then re-fetched (the
 * server already has the mutation by then), up to MAX_SYNC_ATTEMPTS times.
 * A logout or account switch mid-fetch ends the sync — the new owner's own
 * sync takes over.
 */
export async function syncFromServer(): Promise<void> {
  const owner = serverOwner;
  for (let attempt = 0; attempt < MAX_SYNC_ATTEMPTS; attempt++) {
    const startVersion = serverVersion;
    let resp: Awaited<ReturnType<typeof warriorsApi.listWarriors>>;
    try {
      resp = await warriorsApi.listWarriors(1, 100);
    } catch {
      // Offline / 5xx — keep showing the cached list.
      return;
    }
    if (serverOwner !== owner) return;
    if (serverVersion !== startVersion) continue;
    serverWarriors = resp.warriors.map((w) => ({
      id: `server:${w.id}`,
      label: w.name,
      source: w.source,
      isPreset: false,
    }));
    emit();
    return;
  }
}

// Server mutations capture the owner before awaiting the API and drop the
// local apply if the account changed meanwhile, so one user's result never
// lands in another user's list or persisted cache. The server-side change
// itself still happened under the original session; the next sync for that
// user will show it.

export async function createServerWarrior(label: string, source: string): Promise<Warrior> {
  const owner = serverOwner;
  const sw = await warriorsApi.createWarrior(label, source);
  const w: Warrior = { id: `server:${sw.id}`, label: sw.name, source: sw.source, isPreset: false };
  if (serverOwner !== owner) return w;
  serverWarriors = [...serverWarriors, w];
  serverVersion++;
  emit();
  return w;
}

export async function updateServerWarrior(
  localId: string,
  patch: { label?: string; source?: string },
): Promise<void> {
  const serverId = localId.replace('server:', '');
  const apiPatch: { name?: string; source?: string } = {};
  if (patch.label !== undefined) apiPatch.name = patch.label;
  if (patch.source !== undefined) apiPatch.source = patch.source;
  const owner = serverOwner;
  const sw = await warriorsApi.updateWarrior(serverId, apiPatch);
  if (serverOwner !== owner) return;
  serverWarriors = serverWarriors.map((w) =>
    w.id === localId ? { ...w, label: sw.name, source: sw.source } : w,
  );
  serverVersion++;
  emit();
}

export async function deleteServerWarrior(localId: string): Promise<void> {
  const serverId = localId.replace('server:', '');
  const owner = serverOwner;
  await warriorsApi.deleteWarrior(serverId);
  if (serverOwner !== owner) return;
  serverWarriors = serverWarriors.filter((w) => w.id !== localId);
  serverVersion++;
  emit();
}
