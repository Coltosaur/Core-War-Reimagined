#!/usr/bin/env bash
#
# `docker compose` for the prod stack, pinned to the backend image that
# is actually deployed. CI copies this to ~/corewar/compose.sh on the
# droplet; use it for every manual compose command there:
#
#     ~/corewar/compose.sh ps
#     ~/corewar/compose.sh logs -f backend
#
# docker-compose.prod.yml refuses to run without BACKEND_IMAGE_TAG. A
# plain `docker compose up -d` used to fall back to the droplet's local
# `:latest`, which CI never pulls, so it started a months-old backend
# that refused the newer database schema. This script fills the tag in
# from .backend-image-tag, which every deploy writes. An explicit
# BACKEND_IMAGE_TAG in the environment still wins.

set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")"

if [[ -z "${BACKEND_IMAGE_TAG:-}" ]]; then
    if [[ ! -s .backend-image-tag ]]; then
        echo "ERROR: $(pwd)/.backend-image-tag is missing or empty, and" >&2
        echo "       BACKEND_IMAGE_TAG isn't set. Set it to the tag to run," >&2
        echo "       e.g. BACKEND_IMAGE_TAG=master-abcdef1 $0 $*" >&2
        exit 1
    fi
    BACKEND_IMAGE_TAG="$(<.backend-image-tag)"
fi
export BACKEND_IMAGE_TAG

exec docker compose -f docker-compose.prod.yml --env-file .env.production "$@"
