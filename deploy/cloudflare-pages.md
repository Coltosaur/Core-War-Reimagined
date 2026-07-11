# Frontend deploy: Cloudflare Pages runbook

Companion to [`deploy/README.md`](README.md). That doc covers the backend
(DigitalOcean droplet + Caddy). This one covers the frontend (static + Wasm
bundle on Cloudflare Pages).

The two are independent — Pages can deploy before the droplet exists; the
frontend will just hit a non-resolving API host until the backend is up.

## Prerequisites

- Free Cloudflare account.
- Frontend code on master (Pages auto-deploys from a branch).
- Backend hostname decided (`api.corewar.example.com`) — the
  frontend's `VITE_API_URL` env var points here.

## 1. Create the Pages project

Cloudflare dashboard → **Workers & Pages → Create application → Pages → Connect to Git**.

1. Authorize Cloudflare's GitHub app for the `Coltosaur/Core-War-Reimagined` repo.
2. Production branch: `master`.
3. **Build configuration:**

   | Setting | Value |
   |---|---|
   | Framework preset | None |
   | Build command | `bash deploy/cloudflare-pages-build.sh` |
   | Build output directory | `frontend/dist` |
   | Root directory | `/` (leave blank) |

4. **Environment variables** (set under "Environment variables → Production"):

   | Variable | Value |
   |---|---|
   | `NODE_VERSION` | `20` |
   | `VITE_API_URL` | `https://api.corewar.example.com` |

Click **Save and Deploy**. The first build takes 4–6 minutes (Rust toolchain
download + wasm-pack download + `cargo build --release` for the engine +
`npm ci` + `vite build`). Subsequent builds reuse Cloudflare's npm cache.

## 2. Verify the first deploy

When the build finishes you'll get a `<project>.pages.dev` URL. Open it:

- The app should load and render the landing page.
- Open DevTools → Network → reload. The `.wasm` file should be served with
  `content-type: application/wasm` (forced by `frontend/public/_headers`).
- Any client-side navigation that creates a fresh URL (e.g. visit
  `/leaderboard` then reload) should still serve `index.html` — confirms
  the SPA fallback in `frontend/public/_redirects` works.
- The browser will try to hit the backend at `VITE_API_URL` and fail until
  the droplet is up — that's expected at this stage.

## 3. Custom domain (`corewar.example.com`)

Cloudflare dashboard → Pages project → **Custom domains → Set up a custom domain**.

1. Enter `corewar.example.com`.
2. Cloudflare gives you a CNAME target like `<project>.pages.dev`.
3. In **Porkbun DNS** for `example.com`:

   | Type | Host | Answer | TTL |
   |---|---|---|---|
   | `CNAME` | `corewar` | `<project>.pages.dev` | 600 |

4. Back in Cloudflare, wait for it to verify (usually under 5 minutes). Pages
   provisions the TLS cert automatically.

When done, `https://corewar.example.com` serves the production frontend.

## 4. Update backend CORS allowlist

The backend's `FRONTEND_URL` env var is the single allowed CORS origin. On
the droplet, edit `~/corewar/.env.production`:

```ini
FRONTEND_URL=https://corewar.example.com
```

Then restart the backend container:

```bash
docker compose -f docker-compose.prod.yml --env-file .env.production restart backend
```

`<project>.pages.dev` itself will get blocked by CORS at this point — that's
intended; only the public hostname should be talking to the API.

## 5. Operations

- **Push to master** → Pages auto-builds and rolls out a new production
  deploy. Build log + preview URL appear under **Deployments**.
- **PR branches** get **preview deployments** at
  `<branch>.<project>.pages.dev`. They share the same `VITE_API_URL` (prod
  backend) unless you override at preview-environment scope. Preview deploys
  will fail CORS unless you broaden the backend allowlist — fine to leave
  blocked since previews are for visual review, not full E2E.
- **Roll back** in seconds via the **Deployments → ⋯ → Rollback** menu;
  no code change needed.

## Build script

The build is driven by [`deploy/cloudflare-pages-build.sh`](cloudflare-pages-build.sh).
It pins wasm-pack to the same version as the CI workflow
(`.github/workflows/ci.yml`) so the wasm-opt failure modes we hit in #57
stay fixed. Bump both at once if you ever change versions.
