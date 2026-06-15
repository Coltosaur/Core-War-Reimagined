#!/usr/bin/env bash
#
# Cloudflare Pages build script.
#
# Cloudflare Pages doesn't know how to drive a Rust + Wasm + Vite build on
# its own, so we run all five steps here:
#   1. Install rustup + a stable Rust toolchain (the Pages build image ships
#      with Node but NOT Rust, so we bootstrap it ourselves).
#   2. Add the wasm32-unknown-unknown target.
#   3. Download a pinned wasm-pack binary (matches the version pinned in
#      .github/workflows/ci.yml — older wasm-pack ships a too-old
#      wasm-opt that can't parse modern Rust Wasm output).
#   4. Build the engine to Wasm via wasm-pack (writes engine/pkg/).
#   5. Install frontend deps (npm picks up the engine pkg via the
#      `file:../engine/pkg` dep) and run `vite build` → frontend/dist/.
#
# Configure in Cloudflare Pages dashboard:
#   Framework preset:        None
#   Build command:           bash deploy/cloudflare-pages-build.sh
#   Build output directory:  frontend/dist
#   Root directory:          /
#   Environment variables:
#       NODE_VERSION=20
#       VITE_API_URL=https://api.corewar.coltcampbell.dev

set -euo pipefail

# Matches .github/workflows/ci.yml — keep in sync when bumping.
WASM_PACK_VERSION="v0.13.1"
WASM_PACK_TARBALL="wasm-pack-${WASM_PACK_VERSION}-x86_64-unknown-linux-musl"

echo "==> Pages build env: $(uname -a)"
echo "==> node: $(node --version 2>/dev/null || echo MISSING)"

# Install rustup + stable toolchain (minimal profile = rustc + cargo only,
# no docs/rust-src, fastest install). The `-y` skips the interactive prompt.
if ! command -v rustup >/dev/null 2>&1; then
    echo "==> Installing rustup + stable Rust (minimal profile)"
    curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs \
        | sh -s -- -y --default-toolchain stable --profile minimal --no-modify-path
    # rustup installs to $HOME/.cargo by default. Source the env so cargo/rustc
    # land on PATH for the rest of this script.
    # shellcheck disable=SC1091
    . "$HOME/.cargo/env"
else
    echo "==> rustup already present"
fi
echo "==> rustc: $(rustc --version)"

echo "==> Adding wasm32 target"
rustup target add wasm32-unknown-unknown

echo "==> Installing wasm-pack ${WASM_PACK_VERSION}"
TMPDIR="$(mktemp -d)"
curl -fsSL "https://github.com/rustwasm/wasm-pack/releases/download/${WASM_PACK_VERSION}/${WASM_PACK_TARBALL}.tar.gz" \
    | tar -xzC "$TMPDIR"
export PATH="${TMPDIR}/${WASM_PACK_TARBALL}:$PATH"
wasm-pack --version

echo "==> Building Wasm engine (release)"
( cd engine && wasm-pack build --target web --release )

echo "==> Installing frontend deps + building"
( cd frontend && npm ci && npm run build )

echo "==> Done. Output at frontend/dist/"
