#!/usr/bin/env bash
#
# Cloudflare Pages build script.
#
# Cloudflare Pages doesn't know how to drive a Rust + Wasm + Vite build on
# its own, so we run all four steps here:
#   1. Add the wasm32-unknown-unknown target to the Pages build image's
#      pre-installed Rust toolchain.
#   2. Download a pinned wasm-pack binary (matches the version pinned in
#      .github/workflows/ci.yml — older wasm-pack ships a too-old
#      wasm-opt that can't parse modern Rust Wasm output).
#   3. Build the engine to Wasm via wasm-pack (writes engine/pkg/).
#   4. Install frontend deps (npm picks up the engine pkg via the
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
echo "==> rustc: $(rustc --version 2>/dev/null || echo MISSING)"
echo "==> node: $(node --version 2>/dev/null || echo MISSING)"

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
