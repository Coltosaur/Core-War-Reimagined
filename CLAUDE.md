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
- Modifiers for `DJN` / `JMZ` / `JMN` / `SLT`: only `.A` / `.B` / `.AB` / `.BA` (multi-field variants panic — they need a separate semantics decision about how the "jump/skip" condition applies when both fields are involved).
- Modifiers for `SEQ` / `SNE`: only `.I` (full-instruction comparison). Field-wise variants panic. (Note: `SLT` cannot use `.I` because there's no defined ordering for full instructions, so `SLT` is single-field-only.)
- The opcode `Seq` was renamed from the older `Cmp` (ICWS '88 name) to align with ICWS '94. There is no longer an `Opcode::Cmp` variant.
- `SEQ` / `SNE` / `SLT` introduce the **skip-next-instruction** primitive — a conditional that advances PC by 2 instead of 1, distinct from a JMP because there's no target operand.
- `DIV` and `MOD` introduce the **only opcode-internal failure mode**: a divide-by-zero kills the executing process exactly as if it had executed a `DAT`. Implemented by having `arithmetic_op`'s closure return `Option<i32>` — `None` aborts the operation without writing back, and the opcode arm skips enqueueing the next PC.
- The four side-effecting addressing modes (`{ } < >`) all share the same shape: read the intermediate cell, mutate the selected field (decrement before address calc OR increment after), write the intermediate back. This is why `resolve()` takes `&mut Core` rather than `&Core`.
- Modifier variants that aren't yet implemented panic with a clear "not yet implemented" message rather than silently no-op-ing. New modifiers should be added one at a time, each with a focused unit test, plus a full-warrior integration test when a new canonical warrior becomes runnable.

The `resolve()` function takes `&mut Core` rather than `&Core` so that addressing modes with side effects (predecrement, postincrement) can mutate the intermediate cell during resolution. All four side-effecting modes (`{ } < >`) follow the same pattern: read the intermediate, mutate the selected field, write back.

**Parser** (`engine/src/parser.rs`): converts Redcode text source into a `ParsedWarrior` (instructions + start offset + name/author metadata) via `parse_warrior(source: &str) -> Result<ParsedWarrior, ParseError>`. Two-pass for forward-label support: pass 1 walks lines and assigns each instruction a sequential offset while building a label → offset map; pass 2 parses operand bodies and resolves labels to relative offsets. Implements default modifier inference per ICWS '94 §A.2.1, single-operand `DAT`/`NOP` (becomes `(#0, #operand)`), single-operand jumps (becomes `(operand, $0)`), comments and metadata extraction (`;name`, `;author`), and `ORG` / `END` pseudo-ops. Deferred features: `EQU` constants, arithmetic expressions in operand values, multiple warriors per file. Load a parsed warrior into a battle via `MatchState::load_warrior(id, &parsed, base_address)`.

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

**Frontend ↔ Engine:** Frontend imports the wasm-pack output as a JS module (`--target web`). The visualizer plan is PixiJS for the 8000-cell memory grid; code editor is Monaco with custom Redcode syntax.

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
npm install   # first time
npm run dev

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
