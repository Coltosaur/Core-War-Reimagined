# Core War Reimagined

A modernized web rebuild of the 1984 programming game **Core War**. Players write warriors in **Redcode**, an assembly-like language, and battle them inside **MARS** (Memory Array Redcode Simulator) — a virtual machine implemented in Rust and compiled to WebAssembly so it runs at near-native speed directly in the browser.

The engine implements the **complete ICWS '94 specification**: all 16 opcodes, all 8 addressing modes, all 7 modifiers, and a full Redcode parser with labels, EQU macros, and arithmetic expressions.

## Architecture

```
engine/       Rust crate → WebAssembly (runs in the browser, not the server)
frontend/     React 18 + TypeScript + Vite (imports the Wasm module)
backend/      Rust (axum + socketioxide + tokio) API server
```

- The **engine** compiles to Wasm via `wasm-pack` and is imported by the frontend as a JS module. It handles parsing, simulation, and state queries entirely client-side.
- The **frontend** provides a Monaco-based Redcode editor, a battle visualizer, annotated classic warriors, and a learning page.
- The **backend** handles auth, persistence, and (eventually) matchmaking and live battle streaming over Socket.IO. For ranked play it will also run the engine as a native Rust crate dependency for server-side validation.

## Prerequisites

Install the following before setting up the project:

| Tool | Version | Install |
|------|---------|---------|
| **Rust** (stable) | latest | [rustup.rs](https://rustup.rs) |
| **wasm32 target** | — | `rustup target add wasm32-unknown-unknown` |
| **wasm-pack** | 0.13+ | `cargo install wasm-pack` |
| **Node.js** | 18+ | [nvm](https://github.com/nvm-sh/nvm) recommended |
| **npm** | 9+ | Bundled with Node.js |
| **Docker** | 20+ | [Docker Desktop](https://www.docker.com/products/docker-desktop/) or Rancher Desktop |
| **Docker Compose** | v2+ | Bundled with Docker Desktop |

Optional but recommended:

| Tool | Purpose | Install |
|------|---------|---------|
| **cargo-watch** | Auto-rebuild on file changes | `cargo install cargo-watch` |

## Getting Started

### 1. Clone the repo

```bash
git clone https://github.com/Coltosaur/Core-War-Reimagined.git
cd Core-War-Reimagined
```

### 2. Start Postgres and Redis

```bash
docker compose up -d
```

This starts PostgreSQL 16 on port `5432` and Redis 7 on port `6379` with default credentials (`corewar`/`corewar`/`corewar`).

### 3. Set up the backend environment

Create `backend/.env` if it doesn't exist:

```bash
cat > backend/.env << 'EOF'
DATABASE_URL=postgresql://corewar:corewar@localhost:5432/corewar
REDIS_URL=redis://localhost:6379
FRONTEND_URL=http://localhost:5173
PORT=3001
JWT_SECRET=your-secret-here-must-be-at-least-32-bytes
EOF
```

`JWT_SECRET` must be at least 32 bytes. The backend will refuse to start if it's missing or too short.

### 4. Build the Wasm engine

```bash
cd engine
wasm-pack build --target web
cd ..
```

This must be done **before** `npm install` in the frontend — the frontend depends on the Wasm package output at `engine/pkg/`.

### 5. Install frontend dependencies

```bash
cd frontend
npm install
cd ..
```

### 6. Start the backend

```bash
cd backend
cargo run
```

On first run, the backend automatically connects to Postgres and applies database migrations. You should see:

```
INFO core_war_backend::db: connected to postgres
INFO core_war_backend::db: migrations applied
INFO core_war_backend: listening on 0.0.0.0:3001
```

### 7. Start the frontend dev server

In a separate terminal:

```bash
cd frontend
npm run dev
```

The frontend is now available at [http://localhost:5173](http://localhost:5173).

## Running Tests

### Engine tests

The engine has both unit tests (in `vm.rs` and `parser.rs`) and integration tests (in `engine/tests/`) that load `.red` warrior files through the parser:

```bash
cd engine
cargo test
```

### Backend tests

The backend has unit tests for config validation and error responses. These don't require a running database:

```bash
cd backend
cargo test
```

### Running all tests

From the repo root:

```bash
(cd engine && cargo test) && (cd backend && cargo test)
```

## Development Workflow

For active development, run these four processes in separate terminals:

| Terminal | Directory | Command | Purpose |
|----------|-----------|---------|---------|
| 1 | repo root | `docker compose up` | Postgres + Redis |
| 2 | `engine/` | `cargo watch -s 'wasm-pack build --target web'` | Auto-rebuild Wasm on Rust changes |
| 3 | `backend/` | `cargo watch -x run` | Auto-restart backend on Rust changes |
| 4 | `frontend/` | `npm run dev` | Vite dev server with HMR |

After engine Rust changes, you need to run `npm install` in `frontend/` to pick up the new Wasm output (the `file:` dependency is a copy, not a live symlink).

## Production Builds

```bash
# Engine (optimized for size)
cd engine && wasm-pack build --target web --release

# Frontend (outputs to frontend/dist/)
cd frontend && npm run build

# Backend
cd backend && cargo build --release
# Binary at backend/target/release/core-war-backend
```

## Linting and Formatting

```bash
# Rust (works in both engine/ and backend/)
cargo fmt
cargo clippy --all-targets -- -D warnings
```

No frontend linter is configured yet.

## License

See [LICENSE](LICENSE) for details.
