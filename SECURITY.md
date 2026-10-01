# Security Policy

## Reporting a vulnerability

**Please don't open a public issue for security problems.** Report them
privately instead:

1. Go to the repository's **Security** tab.
2. Click **Report a vulnerability**.

Or use this direct link:
<https://github.com/Coltosaur/Core-War-Reimagined/security/advisories/new>

Only the maintainer can see the report. Please include:

- what you found and where (URL, endpoint, Socket.IO event, or file)
- steps to reproduce
- what an attacker could do with it

I'm a solo maintainer, so I can't promise a fixed response time, but I'll
acknowledge reports as soon as I can, keep you updated while it's being
fixed, and credit you in the advisory if you'd like.

## Scope

In scope:

- the live site, <https://corewar.coltcampbell.dev>
- the API, <https://api.corewar.coltcampbell.dev>, including Socket.IO
- the code in this repository

Out of scope:

- denial-of-service through traffic volume
- social engineering or phishing
- issues in third-party services (GitHub, Cloudflare, DigitalOcean) —
  report those to the vendor
- automated scanner output without a demonstrated impact

A bug where a single small request causes disproportionate server load
**is** in scope; that's a code problem, not a traffic-volume one.

## Testing guidelines

Good-faith research is welcome. Please:

- test only against accounts you created yourself
- don't access, modify, or delete other users' data
- don't degrade the service for other players
- give me a reasonable chance to fix the issue before disclosing it
  publicly

## Supported versions

Only the current `master` branch and the live deployment built from it
receive security fixes.
