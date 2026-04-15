# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

A modernized web rebuild of the 1984 programming game **Core War**. Players write **Redcode** warriors that battle inside **MARS** (Memory Array Redcode Simulator), a virtual machine implemented in Rust and compiled to WebAssembly.

The repo is early-stage. The frontend is a stub (`<h1>Core War</h1>`); the backend is a working axum + socketioxide skeleton with a `/health` endpoint and Socket.IO connect/disconnect handlers but no auth, persistence, or matchmaking yet; the engine has a working executor for the canonical Imp, Dwarf, multi-process imp rings (via SPL), Mice-style replicators, and simple linear scanners — covering all three classical warrior strategies (stones, papers, scanners) — implements the **complete ICWS '94 opcode and addressing-mode set**, and **has a working Redcode parser** (`parser::parse_warrior`) that loads warriors from text source. The remaining gaps before full ICWS '94 conformance are the multi-field modifier variants for the jump/skip opcodes; everything else is in place.

**Engine implementation status (see `engine/src/vm.rs`):**
- **All 16 ICWS '94 opcodes** implemented: `DAT`, `MOV`, `ADD`, `SUB`, `MUL`, `DIV`, `MOD`, `JMP`, `JMZ`, `JMN`, `DJN`, `SPL`, `SEQ`, `SNE`, `SLT`, `NOP`.
- **All 8 ICWS '94 addressing modes** implemented: `Immediate`, `Direct`, `AIndirect`, `BIndirect`, `APredecrement`, `BPredecrement`, `APostincrement`, `BPostincrement`.
- The opcode and addressing-mode matches in `execute()` and `resolve()` are both **exhaustive** (no catch-all arm). Adding a new variant to either enum will fail to build until it's handled — this is deliberate, and replaced the earlier "panic on unimplemented" catch-all now that nothing is unimplemented.
- Modifiers for `MOV` / `ADD` / `SUB` / `MUL` / `DIV` / `MOD`: all seven (`A`, `B`, `AB`, `BA`, `F`, `X`, `I`) via the shared `arithmetic_op` + `modifier_field_pairs` helpers. The five arithmetic opcodes share a single closure-driven helper so each match arm is essentially `arithmetic_op(..., |d, s| Some(d OP s))`.
- Modifiers for `DJN` / `JMZ` / `JMN`: **all seven**. `.F`/`.X`/`.I` operate on both fields — DJN decrements both and jumps if either is nonzero; JMZ jumps only when both are zero; JMN jumps when either is nonzero.
- Modifiers for `SEQ` / `SNE`: **all seven**. `.F` requires both field-pairs to match/differ; `.X` uses cross-field pairing; `.I` is whole-instruction equality. SNE uses OR (skip if ANY pair differs), the De Morgan inverse of SEQ's AND.
- Modifiers for `SLT`: **all seven**. `.F`/`.I` require both pairs to satisfy `<`; `.X` uses cross-field pairing. `.I` is treated as `.F` (no ordering defined for full instructions).
- **No `unimplemented!()` calls remain in the engine.** Every opcode × modifier × addressing-mode combination is handled.
- The opcode `Seq` was renamed from the older `Cmp` (ICWS '88 name) to align with ICWS '94. There is no longer an `Opcode::Cmp` variant.
- `SEQ` / `SNE` / `SLT` introduce the **skip-next-instruction** primitive — a conditional that advances PC by 2 instead of 1, distinct from a JMP because there's no target operand.
- `DIV` and `MOD` introduce the **only opcode-internal failure mode**: a divide-by-zero kills the executing process exactly as if it had executed a `DAT`. Implemented by having `arithmetic_op`'s closure return `Option<i32>` — `None` aborts the operation without writing back, and the opcode arm skips enqueueing the next PC.
- The four side-effecting addressing modes (`{ } < >`) all share the same shape: read the intermediate cell, mutate the selected field (decrement before address calc OR increment after), write the intermediate back. This is why `resolve()` takes `&mut Core` rather than `&Core`.
- **Process count limit:** `MatchState` has a `max_processes` field (default 8000, configurable via `set_max_processes`). SPL silently refuses to spawn a new process when the warrior is at the limit — the continuing process still runs.

Deferred features: multiple warriors per file (`FOR`/`ROF` loops). Load a parsed warrior into a battle via `MatchState::load_warrior(id, &parsed, base_address)`.

**Parser EQU support:** The parser handles `name EQU value` pseudo-ops. EQU defines a text substitution consumed during operand value parsing. EQU names are case-sensitive (like labels), checked before labels in the resolution chain, and do not count as instructions. Duplicate EQU names produce a `DuplicateLabel` error.

**Parser expression support:** Operand values accept full arithmetic expressions — literals, labels, and EQU identifiers combined with `+ - * / %` (standard precedence) and parenthesized sub-expressions. Unary `+`/`-` work as expected. Labels inside expressions resolve to their PC-relative offset, so `target + 2` means "two past target" and `end - start` is the constant distance between two labels. EQU values are themselves evaluated as expressions, with cycle detection — `a EQU b + 1` + `b EQU a + 1` errors out rather than recursing forever. Division or modulo by zero in an expression is a parse-time `SyntaxError`, separate from the runtime divide-by-zero on the `DIV`/`MOD` opcodes.

The `resolve()` function takes `&mut Core` rather than `&Core` so that addressing modes with side effects (predecrement, postincrement) can mutate the intermediate cell during resolution. All four side-effecting modes (`{ } < >`) follow the same pattern: read the intermediate, mutate the selected field, write back.

**Parser** (`engine/src/parser.rs`): converts Redcode text source into a `ParsedWarrior` (instructions + start offset + name/author metadata) via `parse_warrior(source: &str) -> Result<ParsedWarrior, ParseError>`. Two-pass for forward-label support: pass 1 walks lines and assigns each instruction a sequential offset while building a label → offset map; pass 2 parses operand bodies and resolves labels to relative offsets. Implements default modifier inference per ICWS '94 §A.2.1, single-operand `DAT`/`NOP` (becomes `(#0, #operand)`), single-operand jumps (becomes `(operand, $0)`), comments and metadata extraction (`;name`, `;author`), and `ORG` / `END` pseudo-ops. Deferred features: multiple warriors per file (`FOR`/`ROF` loops). Load a parsed warrior into a battle via `MatchState::load_warrior(id, &parsed, base_address)`.

## Engine tests

Two layers, deliberately:

- **Unit tests** in `engine/src/vm.rs` and `engine/src/parser.rs` (`#[cfg(test)] mod tests`). These can see private items and test internal correctness — invariants of `step()`, the side effects in `resolve()`, modifier dispatch tables, parser pass internals. The `vm.rs` tests are also where the canonical warriors live in their hand-built `Instruction`-literal form, which is the source of truth that the parser tests check against.
- **Integration tests** in `engine/tests/canonical_warriors.rs` with `.red` warrior files in `engine/tests/warriors/`. Each `.rs` file in `tests/` is compiled as a *separate binary* that links against `core_war_engine` as a downstream consumer would and can only use items that are `pub use`'d from `lib.rs` — these tests catch any "I forgot to make this public" bugs the in-crate unit tests can't. The convention here is that **none of these tests construct an `Instruction` literal directly**; every warrior, including markers planted into core, comes through `parse_warrior`. That makes the file a clean usage example for the eventual frontend and backend code.

To add a new canonical warrior: drop the `.red` file in `engine/tests/warriors/`, add a `const NAME: &str = include_str!("warriors/name.red");` line at the top of `canonical_warriors.rs`, and write a `#[test]` that loads it via `parse_warrior` and `MatchState::load_warrior`.

`MatchState::result()` returns a `MatchResult` enum (`Ongoing` / `Victory { winner_id }` / `Tie` / `AllDead`) for queries about who's won. It is purely a query — calling `result()` does not stop the simulation, and `step()` will keep executing the surviving warrior even after `Victory` is reported. The `Tie` and `AllDead` variants are kept distinct because they encode different end-state diagnostics (step limit hit vs. mutual death) even though both are "no winner" for scoring purposes.

## Architecture

Three independent components, **two** deployment services (frontend static + backend API):

```
engine/    Rust crate → Wasm (wasm-bindgen, wasm-pack). NOT a server process.
frontend/  React 18 + TypeScript + Vite. Imports the Wasm module directly.
backend/   Rust (axum + socketioxide + tokio). Postgres + Redis (planned).
```

**Critical architectural rule:** The Rust engine compiles to Wasm and runs **in the browser** alongside the frontend — it is not a server-side process and is not bundled into the backend container. The backend exists for auth, matchmaking, persistence, and live-battle WebSocket streaming for ranked play. For ranked-match validation, the backend will eventually call the engine as a native Rust crate dependency (`cargo add core-war-engine` from `backend/`), avoiding any FFI bridge.

**Backend stack:** Rust binary crate `core-war-backend`. Axum 0.7 for HTTP routing, socketioxide 0.15 for Socket.IO (wire-compatible with the Socket.IO JS client), tower-http for CORS, tokio runtime, dotenvy for `.env`, tracing/tracing-subscriber for logging. Entry point: `backend/src/main.rs`. The backend was originally scaffolded in Python (FastAPI + python-socketio) and swapped to Rust early so the engine could be a direct crate dependency for server-side battle validation. Planning docs (`corewars_project_summary.md`) still reference the old Python stack — ignore those.

**Future backend deps (add when first used, not preemptively):** `sqlx` (Postgres, with compile-time-checked queries), `redis` with `tokio-comp` (Redis), `apalis` (Redis-backed job queue, equivalent to Python's `arq`), `jsonwebtoken` (JWT), `argon2` (password hashing).

**Frontend ↔ Engine:** Frontend imports the wasm-pack output as a JS module (`--target web`). The engine's `wasm` module (`engine/src/wasm.rs`, compiled only on `wasm32`) provides `#[wasm_bindgen]` wrapper types (`MatchState`, `ParsedWarrior`) and free functions (`parseWarrior`, `engineVersion`) that adapt the native Rust API for JS consumption. Key methods:
- `parseWarrior(source)` → `ParsedWarrior` (or throws a JS error string on parse failure)
- `new MatchState(coreSize, maxSteps)` → `MatchState`
- `matchState.loadWarrior(id, warrior, baseAddress)` — bridge from parser to executor
- `matchState.step()` / `matchState.stepN(n)` — advance the simulation (use `stepN` to avoid N individual wasm boundary crossings per frame)
- `matchState.resultCode()` → `0=Ongoing, 1=Victory, 2=Tie, 3=AllDead`; `resultWinnerId()` → winner id or -1
- `matchState.coreOpcodes()` → `Uint8Array` of opcode bytes (fast render path for the PixiJS grid)
- `matchState.coreSnapshot()` → `Uint32Array` with two words per cell (packed metadata + signed i16 field values for the detail/inspector view)
- Individual cell reads: `cellOpcode(addr)`, `cellModifier(addr)`, `cellAMode(addr)`, `cellAValue(addr)`, `cellBMode(addr)`, `cellBValue(addr)`
- Warrior queries: `warriorCount()`, `warriorIsAlive(idx)`, `warriorProcessCount(idx)`, `warriorProcessPcs(idx)` → `Uint32Array`

The visualizer plan is PixiJS for the 8000-cell memory grid; code editor is Monaco with custom Redcode syntax. Enums are encoded as `u8` discriminants (all three — `Opcode`, `Modifier`, `AddressMode` — have `#[repr(u8)]` for zero-cost casting).

**Frontend ↔ Backend:** Socket.IO over WebSockets for live battle streaming, plus REST for auth/leaderboards. CORS is keyed off `FRONTEND_URL` (defaults to `http://localhost:5173`).

## Common commands

Local dev requires four things running in parallel:

```bash
# 1. Postgres + Redis (from repo root)
docker compose up

# 2. Build the Wasm engine (from engine/) — re-run after Rust changes
wasm-pack build --target web
# or: cargo watch -s 'wasm-pack build --target web'

# 3. Frontend dev server (from frontend/) — Vite on http://localhost:5173
npm install   # first time, OR after wasm-pack rebuild
npm run dev
# NOTE: The frontend depends on `core-war-engine` via `"file:../engine/pkg"`.
# You must run `wasm-pack build --target web` in engine/ BEFORE `npm install`
# in frontend/, otherwise npm will fail because `../engine/pkg` doesn't exist.
# After any engine Rust change, re-run wasm-pack and then `npm install` to
# pick up the new wasm output (the file: dep is a copy, not a live symlink).

# 4. Backend API (from backend/) — axum on http://localhost:3001
cargo run
# or with auto-reload (requires cargo-watch — installed globally):
cargo watch -x run
```

### Smoke-testing the backend

`frontend/scripts/test-backend.mjs` is a small Node script that hits `/health` and opens a Socket.IO connection against the backend, verifying both transports end-to-end. Use it any time the backend wiring is in question:

```bash
# (with the backend running)
node frontend/scripts/test-backend.mjs
```

It exits 0 on success, 1 on any failure, and prints the assigned `sid` so you can cross-reference it against the backend's `client connected: <sid>` log line — matching SIDs prove you're talking to the process you think you are (this matters: a stale uvicorn from a previous session once silently answered this test).

Frontend production build: `npm run build` (outputs to `frontend/dist/`); preview with `npm run preview`.

Engine release build: `wasm-pack build --target web --release` (uses `opt-level = "s"` for size).

Backend release build: `cargo build --release` from `backend/`. Output binary at `backend/target/release/core-war-backend`.

There are **no tests configured yet** in any of the three components. Rust formatting/linting are available out of the box: `cargo fmt` and `cargo clippy --all-targets -- -D warnings` work in both `engine/` and `backend/` without any config. There's no frontend linter set up yet.

## Environment

- All dev happens inside **WSL2 Ubuntu** on Windows 11. Keep files in the Linux filesystem (`~/dev/...`), never `/mnt/c/...`, for performance and to avoid file-watcher issues with Vite and `cargo watch`.
- Backend reads `backend/.env` via `python-dotenv`. Defaults assume Postgres at `localhost:5432` (user/pass/db all `corewar`) and Redis at `localhost:6379`, matching `docker-compose.yml`.
- Backend port is `3001`; frontend dev server is `5173`. Don't change one without updating CORS (`FRONTEND_URL`) and any frontend API base URL.

## Key concepts

- **MARS** — Memory Array Redcode Simulator; the VM battles run inside.
- **Redcode** — assembly-like language warriors are written in.
- **Wasm** — compiled-from-Rust binary that runs client-side in the browser at near-native speed.
