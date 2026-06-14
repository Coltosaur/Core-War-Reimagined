#!/usr/bin/env bash
#
# Manual deploy script for the prod-mirror Docker Compose stack.
#
# Usage:
#     ./deploy/deploy.sh user@host
#
# What it does:
#   1. Rsyncs the project tree to ~/corewar/ on the target host,
#      excluding build artifacts and any local .env* files.
#   2. SSHes in and runs `docker compose up -d --build` on
#      docker-compose.prod.yml using the .env.production file that
#      already exists on the droplet (NOT rsynced from your laptop).
#
# Prerequisites on the target:
#   - SSH key auth working (no password prompts).
#   - Docker Engine + compose plugin installed (see deploy/README.md).
#   - ~/corewar/.env.production created and filled in.
#
# Exits non-zero on any failure so it's safe to chain in a script.

set -euo pipefail

if [[ $# -ne 1 ]]; then
    echo "Usage: $0 <user@host>" >&2
    exit 1
fi

TARGET="$1"
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REMOTE_PATH="corewar"  # relative to the remote user's $HOME

echo "==> Syncing $REPO_ROOT -> $TARGET:~/$REMOTE_PATH/"

# Rsync filter rules are evaluated in order; first match wins.
# We include .env.production.example explicitly so the runbook's
# `cp .env.production.example .env.production` step has something to
# copy on the droplet. Real .env files are excluded so secrets never
# leave the laptop — and excluded files are not deleted by --delete
# either, so the droplet's .env.production survives redeploys.
rsync -azh --delete \
    --include '.env.production.example' \
    --exclude '.env' \
    --exclude '.env.*' \
    --exclude '.git' \
    --exclude '.github' \
    --exclude 'target' \
    --exclude 'node_modules' \
    --exclude 'frontend/dist' \
    --exclude 'engine/pkg' \
    --exclude '.playwright-mcp' \
    --exclude '.mcp.json' \
    --exclude '.vscode' \
    --exclude '.idea' \
    --exclude '.DS_Store' \
    "$REPO_ROOT/" "$TARGET:$REMOTE_PATH/"

echo "==> Rebuilding and restarting prod stack on $TARGET"

# Run a single remote bash session so we can keep -euo pipefail
# behavior on the droplet side.
ssh "$TARGET" 'bash -s' <<'REMOTE'
set -euo pipefail
cd ~/corewar

if [[ ! -f .env.production ]]; then
    echo "ERROR: ~/corewar/.env.production not found." >&2
    echo "Create it from .env.production.example and fill in secrets:" >&2
    echo "    cp .env.production.example .env.production" >&2
    echo "    \$EDITOR .env.production" >&2
    exit 1
fi

docker compose -f docker-compose.prod.yml --env-file .env.production up -d --build
docker compose -f docker-compose.prod.yml --env-file .env.production ps
REMOTE

echo "==> Done. Smoke-test with: curl -i https://<your-api-host>/health"
