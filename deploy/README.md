# VPS bootstrap & manual deploy runbook

This runbook stands the backend stack up on a fresh DigitalOcean droplet,
hardens it, configures DNS for `api.corewar.coltcampbell.dev`, and walks
through the first deploy and the steady-state redeploy.

The deploy is fully manual at this stage. `deploy/deploy.sh` rsyncs the
project to the droplet and rebuilds the prod compose stack over SSH —
GitHub Actions automation comes in PR 4.

For the **frontend** (Cloudflare Pages) see the sibling runbook
[`deploy/cloudflare-pages.md`](cloudflare-pages.md). Frontend and backend
deploys are independent — Pages can ship before this VPS exists; the app
will just see a non-resolving API host until the droplet is up.

> All `<placeholder>` values are things you fill in. Wherever the runbook
> references `coltcampbell.dev`, replace with your domain if you forked.

## Prerequisites

Before starting:

- DigitalOcean account with a payment method on file.
- Domain `coltcampbell.dev` registered (Porkbun) with DNS management access.
- A dedicated SSH key pair for the droplet — **separate from any key you already use** (e.g. your GitHub key at `~/.ssh/id_ed25519`). Generate one with an explicit filename so the default location isn't clobbered:
  ```bash
  ssh-keygen -t ed25519 -f ~/.ssh/droplet -C "colt@coltcampbell.dev"
  ```
  Subsequent commands assume this key. To skip typing `-i ~/.ssh/droplet` every time, add a `~/.ssh/config` alias once the droplet's hostname resolves:
  ```
  Host droplet api.corewar.coltcampbell.dev
      HostName api.corewar.coltcampbell.dev
      User colt
      IdentityFile ~/.ssh/droplet
      IdentitiesOnly yes
  ```
  Until then (and for the initial `root@` login below), pass `-i ~/.ssh/droplet -o IdentitiesOnly=yes` explicitly.
- Docker installed locally (PR 1's prod-mirror stack already requires this).

## 1. Provision the droplet

DigitalOcean control panel → **Create → Droplets**:

| Setting | Value |
|---|---|
| Region | NYC3 (or SFO3 — pick whichever is closest to your users) |
| OS image | Ubuntu 24.04 (LTS) x64 |
| Plan | Basic → Regular CPU → **$6/mo (1 GB / 1 CPU / 25 GB SSD / 1 TB transfer)** |
| Authentication | SSH key — paste the contents of `~/.ssh/droplet.pub`. Do **not** use a password. |
| IPv6 | Enable (free; you'll add an AAAA record alongside the A in section 6) |
| Hostname | `corewar-prod` |
| Backups | Skip for now ($1.20/mo, add later if you want) |
| Monitoring | Enable (free) |

After creation, copy the droplet's IPv4 (and IPv6 if you want to support it).
The rest of the runbook calls it `<DROPLET_IP>`.

## 2. First SSH + create a non-root user

DigitalOcean drops the SSH key onto `root@<DROPLET_IP>` for the initial
connection. The first thing we do is leave that login behind.

```bash
ssh -i ~/.ssh/droplet -o IdentitiesOnly=yes root@<DROPLET_IP>
```

(The `-i` + `IdentitiesOnly=yes` belt-and-suspenders tells SSH not to try other
keys in your agent first — handy if your default key isn't authorized on the
droplet.)

On the droplet:

```bash
# Patch the base image. If apt prompts about /etc/ssh/sshd_config during the
# openssh-server upgrade, press D to view the diff, then prefer the package
# maintainer's version — section 2 below sets the lockdown lines explicitly.
apt update && apt upgrade -y
apt install -y sudo

# Create a non-root user (replace 'colt' with whatever you prefer)
adduser --disabled-password --gecos "" colt
usermod -aG sudo colt

# Allow colt to sudo without typing a password. The user was created
# --disabled-password so sudo would otherwise refuse. Cloud-init does this
# same trick for default cloud-image users; safe here because SSH is
# key-only and the blast radius if colt is compromised is "the whole
# droplet" with or without a sudo password.
echo 'colt ALL=(ALL) NOPASSWD:ALL' > /etc/sudoers.d/colt
chmod 0440 /etc/sudoers.d/colt
visudo -c    # syntax-check; should print "/etc/sudoers.d/colt: parsed OK"

# Reuse your existing SSH key for the new user
rsync --archive --chown=colt:colt ~/.ssh /home/colt
```

Log out, then SSH back in as the new user from your laptop to confirm it works:

```bash
ssh -i ~/.ssh/droplet -o IdentitiesOnly=yes colt@<DROPLET_IP>
```

If that succeeds, lock down SSH on the droplet:

```bash
sudo sed -i 's/^#\?PermitRootLogin.*/PermitRootLogin no/' /etc/ssh/sshd_config
sudo sed -i 's/^#\?PasswordAuthentication.*/PasswordAuthentication no/' /etc/ssh/sshd_config
sudo systemctl restart ssh
```

Validate from a **separate** terminal that you can still log in as `colt`
before closing the original session.

## 3. Firewall (ufw)

Only SSH, HTTP, and HTTPS should be reachable from the public internet.

```bash
sudo ufw default deny incoming
sudo ufw default allow outgoing
sudo ufw allow OpenSSH
sudo ufw allow 80/tcp
sudo ufw allow 443/tcp
sudo ufw enable
sudo ufw status verbose
```

## 4. fail2ban (SSH brute-force guard)

```bash
sudo apt install -y fail2ban
sudo systemctl enable --now fail2ban
sudo fail2ban-client status sshd
```

Default config is fine — it watches `sshd` and bans IPs that fail auth too
often. With password auth already disabled this is belt-and-suspenders, but
cheap insurance.

## 5. Install Docker Engine

Use Docker's official apt repository — the version in Ubuntu's default repos
lags. The Snap version is broken for our use case (volume permission issues).

```bash
# Repo prereqs
sudo apt install -y ca-certificates curl gnupg
sudo install -m 0755 -d /etc/apt/keyrings
curl -fsSL https://download.docker.com/linux/ubuntu/gpg | \
    sudo gpg --dearmor -o /etc/apt/keyrings/docker.gpg
sudo chmod a+r /etc/apt/keyrings/docker.gpg

echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.gpg] \
https://download.docker.com/linux/ubuntu $(. /etc/os-release && echo $VERSION_CODENAME) stable" | \
    sudo tee /etc/apt/sources.list.d/docker.list > /dev/null

sudo apt update
sudo apt install -y docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin

# Run docker without sudo
sudo usermod -aG docker $USER
```

Log out and back in to pick up the group change, then verify:

```bash
docker run --rm hello-world
docker compose version
```

### 5a. Allocate swap (2 GB)

The $6 droplet has 1 GB RAM. Steady-state operations fit, but the
`deploy/deploy.sh` escape hatch runs `cargo build --release` on-box, which
can OOM during linking. A swap file gives the kernel a pressure-release
valve without changing the plan:

```bash
sudo fallocate -l 2G /swapfile
sudo chmod 600 /swapfile
sudo mkswap /swapfile
sudo swapon /swapfile
echo '/swapfile none swap sw 0 0' | sudo tee -a /etc/fstab
free -h    # verify "Swap:" row shows 2.0Gi
```

### 5b. Log into GitHub Container Registry

The CI workflow publishes the backend image to `ghcr.io/coltosaur/core-war-backend`.
By default, ghcr.io creates packages as **private**, which means the droplet
needs to authenticate before it can pull. (If you'd rather make the package
public, you can skip this whole step — but private is the more defensible
default for portfolio infrastructure.)

Create a Personal Access Token first, on your laptop:

1. GitHub → **Settings → Developer settings → Personal access tokens (classic) → Generate new token**.
2. Note: `corewar-droplet-pull`.
3. Expiration: 1 year (set a calendar reminder to rotate).
4. Scopes: tick **`read:packages`** only.
5. Generate, copy the token (you can't see it again).

Then on the droplet, log in as `colt` (no sudo — Docker reads credentials
from `~/.docker/config.json` under the current user's home):

```bash
echo "<paste-PAT-here>" | docker login ghcr.io -u Coltosaur --password-stdin
```

You should see `Login Succeeded`. Verify the pull path works:

```bash
docker pull ghcr.io/coltosaur/core-war-backend:latest
```

## 6. DNS records at Porkbun

In the Porkbun control panel for `coltcampbell.dev` → **DNS**:

| Type | Host | Answer | TTL |
|---|---|---|---|
| `A` | `api.corewar` | `<DROPLET_IPV4>` | 600 |
| `AAAA` | `api.corewar` | `<DROPLET_IPV6>` (optional) | 600 |

Verify propagation from your laptop (give it 1–5 minutes):

```bash
dig +short api.corewar.coltcampbell.dev
dig +short AAAA api.corewar.coltcampbell.dev
```

Both should return the droplet's addresses before you proceed — Caddy can't
issue a Let's Encrypt cert until the hostname resolves to this box.

## 7. First deploy from your laptop

From the repo root on your laptop:

```bash
./deploy/deploy.sh colt@<DROPLET_IP>
```

That rsyncs the project tree to `~/corewar/` on the droplet. The first run
will fail loudly because `.env.production` doesn't exist on the droplet
yet — that's expected. SSH in and create it:

```bash
ssh colt@<DROPLET_IP>
cd ~/corewar
cp .env.production.example .env.production
nano .env.production    # or vim, whatever
```

Fill in real values. **Use `openssl rand -hex` (not `-base64`) for anything
that lands inside `DATABASE_URL`** — base64 output includes `/`, `+`, and `=`,
all of which break URL parsing (sqlx will reject the URL with
`Configuration(InvalidPort)`).

```ini
BACKEND_DOMAIN=api.corewar.coltcampbell.dev
ACME_EMAIL=colt@coltcampbell.dev
FRONTEND_URL=https://corewar.coltcampbell.dev
JWT_SECRET=<openssl rand -hex 48>
POSTGRES_USER=corewar
POSTGRES_PASSWORD=<openssl rand -hex 24>
POSTGRES_DB=corewar
DATABASE_URL=postgresql://corewar:<same hex password>@postgres:5432/corewar
REDIS_URL=redis://redis:6379
TRUSTED_PROXIES=
```

`POSTGRES_PASSWORD` and the password portion of `DATABASE_URL` MUST match
byte-for-byte. If you change one, change both — and `down -v` the postgres
volume so the new credentials take effect (the volume bakes credentials in
on first init and ignores env-var changes afterward).

Then re-run the deploy from your laptop:

```bash
./deploy/deploy.sh colt@<DROPLET_IP>
```

This time it'll build the backend image on the droplet (~2–3 min on a $6
droplet — the build is CPU-bound on cargo) and bring the stack up.

## 8. Verify TLS

```bash
curl -i https://api.corewar.coltcampbell.dev/health
```

The first request can take **30+ seconds** while Caddy negotiates the
Let's Encrypt cert. Expected response:

```
HTTP/2 200
content-type: application/json
{"status":"ok","database":"ok"}
```

(Older deploys returned just `{"status":"ok"}` — `/health` now also probes
Postgres via a 2s-timeout `SELECT 1` and returns HTTP 503 if the DB is
unreachable.)

If the cert handshake fails:

- `dig +short api.corewar.coltcampbell.dev` must return the droplet IP.
- Port 80 must be reachable from the public internet (Let's Encrypt's
  HTTP-01 challenge uses it). `sudo ufw status` should show 80/tcp ALLOW.
- Inspect Caddy's logs: `docker compose -f docker-compose.prod.yml logs caddy`.

If `/health` returns 503 with `"database":"fail"`, the backend booted but
can't talk to Postgres. Tail the backend logs to see the exact error:

```bash
docker compose -f docker-compose.prod.yml --env-file .env.production logs --tail 80 backend
```

Common causes:
- **`Configuration(InvalidPort)`** — `DATABASE_URL` password contains URL-special
  characters (`/`, `+`, `:`, `@`, `=`). Regenerate with `openssl rand -hex`.
- **`password authentication failed for user "corewar"`** — `POSTGRES_PASSWORD`
  and the password in `DATABASE_URL` disagree, OR a previous `up` initialized
  the postgres volume with different credentials. Fix: nuke the volume with
  `docker compose ... down -v` and re-up.

## 9. Operations cheat sheet

All commands run from `~/corewar/` on the droplet.

```bash
# Tail logs
docker compose -f docker-compose.prod.yml --env-file .env.production logs -f backend

# Restart just the backend (after a config change)
docker compose -f docker-compose.prod.yml --env-file .env.production restart backend

# Stop everything (volumes survive)
docker compose -f docker-compose.prod.yml --env-file .env.production down

# Nuke the database (only if you mean it — volumes go too)
docker compose -f docker-compose.prod.yml --env-file .env.production down -v
```

Steady-state redeploy from your laptop after pushing code (escape hatch
when CI is unavailable — normal redeploys happen automatically via
GitHub Actions, see section 10):

```bash
./deploy/deploy.sh colt@<DROPLET_IP>
```

The script rsyncs (skipping `.env.production` and other excluded paths),
then runs `docker compose up -d --build` over SSH so only changed layers
rebuild.

## 10. Enable CI deploys (deploy-on-merge)

Once the droplet is up and the manual deploy from section 7 worked, wire
up `.github/workflows/deploy-backend.yml` to take over steady-state
deploys. The workflow builds the backend image on every push to master,
pushes it to GitHub Container Registry (ghcr.io), then SSHes into the
droplet and rolls out the new image.

### Package visibility — already handled

Section 5b walked through logging the droplet into GHCR with a
`read:packages` PAT, so a private package is the assumed default. If
you'd rather make the package public (no droplet auth needed, anyone can
pull the compiled image), GitHub → your profile → **Packages** →
`core-war-backend` → **Package settings → Change visibility → Public**.
Source is already public; the binary doesn't expose anything new.

### Required GitHub Actions secrets

Repo → **Settings → Secrets and variables → Actions → Secrets → New
repository secret**:

| Secret | Value |
|---|---|
| `DROPLET_HOST` | Droplet IPv4 (or `api.corewar.coltcampbell.dev` once DNS is in) |
| `DROPLET_USER` | The non-root user from section 2 (e.g. `colt`) |
| `DROPLET_SSH_KEY` | **Private** half of an SSH key whose public half is in the droplet's `~/.ssh/authorized_keys`. Generate a fresh one for CI — don't reuse your personal key: `ssh-keygen -t ed25519 -f ~/.ssh/corewar_deploy -C corewar-deploy`. Copy `~/.ssh/corewar_deploy.pub` to the droplet's `~/.ssh/authorized_keys`, paste the contents of `~/.ssh/corewar_deploy` (the private file) here. |

### Required GitHub Actions variable

Same page → **Variables → New repository variable**:

| Variable | Value |
|---|---|
| `DEPLOY_ENABLED` | `true` |

The workflow's `deploy` job is gated by `vars.DEPLOY_ENABLED == 'true'`,
so until you set this it'll only build + push the image without
attempting to ssh anywhere. Useful for the gap between provisioning the
droplet and being ready to roll out.

### Trigger the first CI deploy

After the secrets and variable are in place:

```
GitHub repo → Actions → "Deploy backend" → Run workflow → master → Run
```

The job log will show the image push, the rsync, and the docker compose
restart. Health check waits up to 60s for the container to report
`healthy` before declaring success.

From then on, every push to master that touches `backend/`, `engine/`,
`Caddyfile`, or `docker-compose.prod.yml` triggers a deploy. PRs build
the image without pushing (so Dockerfile breakage gets caught before
merge).

## 11. Uptime monitoring

External monitor pinging `/health` so you find out about an outage from
a notification, not from a friend trying to use the site. **UptimeRobot**
has the longest-running free tier and is what we'll wire up:

1. Sign up at [uptimerobot.com](https://uptimerobot.com) — free, no card.
2. **+ New Monitor**:
   - Type: HTTPS
   - Friendly name: `Core War backend`
   - URL: `https://api.corewar.coltcampbell.dev/health`
   - Monitoring interval: 5 minutes (free-tier minimum)
3. **Alert contacts**: at least email. Optional: Slack, Telegram, webhooks.
4. **Advanced** (optional but recommended):
   - **Keyword monitoring**: also check the response body for the string
     `"status":"ok"`. Catches the case where `/health` returns 503 with
     `"status":"degraded"` (DB unreachable) but the TLS/proxy layer is fine.

The same pattern will work for the frontend once Cloudflare Pages is up
— add a second monitor for `https://corewar.coltcampbell.dev`.

## What's NOT here (yet)

- **Database backups.** Volumes survive `down`, but the droplet does not.
  A future PR will add either DO managed snapshots ($1.20/mo) or a
  `pg_dump`-to-Backblaze-B2 cron job.
- **Redis check in `/health`.** Currently only Postgres is probed. Adding
  Redis means restructuring `AppState`; tracked as a follow-up.
- **Migrations as an explicit deploy step.** The backend currently runs
  `sqlx::migrate!()` at startup; migration failures surface as backend
  startup errors in `docker compose logs backend` and the CI deploy job's
  60s health-check loop catches them. Splitting into a separate
  pre-startup step is a nice-to-have, not a blocker.
