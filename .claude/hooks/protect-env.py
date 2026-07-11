#!/usr/bin/env python3
"""PreToolUse hook: block reads/writes/searches of .env files.

Allows .env.example / .env.sample / .env.template (docs/templates).
Blocks .env, .env.local, .env.production, .env.*.local, etc.

Covers Read, Edit, Write, NotebookEdit, Grep, and Bash.

Scope / limitations. This is defense-in-depth against accidental or
forgetful access, not an airtight sandbox. Bash commands are checked
via shlex tokenization: a token that is a `.env`-like filename gets
blocked, tokens with internal whitespace (i.e. quoted string content)
are skipped. That means `git commit -m "mentions .env"` passes, but
so does `python -c "print(open('.env').read())"` because we cannot
tell quoted code from quoted text without executing it. Similar gaps
exist for symlinks, `find -exec`, base64-encoded paths, etc. Closing
those requires a real shell parser or full sandboxing; this hook
raises the cost of casual/unintended access without pretending to be
unbypassable.
"""

from __future__ import annotations

import json
import re
import shlex
import sys

ALLOWED_SUFFIXES = (".example", ".sample", ".template")

# A .env-like token: `.env` optionally followed by dotted suffixes.
# Left boundary: start of string OR any char that isn't a word/dot/hyphen — so
# `my.env-thing` and `foo.envconfig` don't match, but `cat .env`, `< .env.local`,
# and `path/to/.env.production` do.
DOTENV_TOKEN = re.compile(r"(?<![\w.-])\.env(?:\.[a-zA-Z0-9_-]+)*")

# Strict filename match — a real dotenv basename, nothing embedded in a
# larger blob (like JSON payloads that happen to contain `.env`).
DOTENV_FILENAME = re.compile(r"^\.env(?:\.[a-zA-Z0-9_-]+)*$")


def basename(path: str) -> str:
    return path.rsplit("/", 1)[-1]


def is_protected_path(path: str) -> bool:
    if not path:
        return False
    base = basename(path)
    if not DOTENV_FILENAME.fullmatch(base):
        return False
    if base.endswith(ALLOWED_SUFFIXES):
        return False
    return True


def bash_offender(cmd: str) -> str | None:
    """Return the first non-allowed .env token in a shell command, or None.

    Uses shlex.split so quoted string content (like `-m "message with .env"`)
    doesn't false-positive: real file arguments cannot contain unescaped
    whitespace, so any token with internal whitespace came from a quoted
    string, not a file reference. Falls back to a raw regex scan if the
    command can't be tokenized (unmatched quotes, heredocs, etc.).
    """
    try:
        tokens = shlex.split(cmd, comments=False, posix=True)
    except ValueError:
        for m in DOTENV_TOKEN.finditer(cmd):
            token = m.group()
            if not token.endswith(ALLOWED_SUFFIXES):
                return token
        return None

    for tok in tokens:
        if any(c.isspace() for c in tok):
            continue
        if is_protected_path(tok):
            return tok
    return None


def pattern_offender(pattern: str) -> str | None:
    for m in DOTENV_TOKEN.finditer(pattern):
        token = m.group()
        if not token.endswith(ALLOWED_SUFFIXES):
            return token
    return None


def deny(reason: str) -> None:
    print(json.dumps({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": reason,
        }
    }))
    sys.exit(0)


def main() -> None:
    try:
        payload = json.load(sys.stdin)
    except json.JSONDecodeError:
        sys.exit(0)

    tool_name = payload.get("tool_name", "")
    tool_input = payload.get("tool_input") or {}

    if tool_name in ("Read", "Edit", "Write"):
        path = tool_input.get("file_path", "") or ""
        if is_protected_path(path):
            deny(
                f"Blocked: '{path}' is a protected secrets file. "
                f"Read the corresponding .example file instead "
                f"(e.g. backend/.env.example)."
            )

    elif tool_name == "NotebookEdit":
        path = tool_input.get("notebook_path", "") or ""
        if is_protected_path(path):
            deny(f"Blocked: '{path}' is a protected secrets file.")

    elif tool_name == "Grep":
        path = tool_input.get("path", "") or ""
        if is_protected_path(path):
            deny(
                f"Blocked: cannot Grep '{path}'. "
                f"Read the corresponding .example file instead."
            )
        for key in ("glob", "include"):
            pat = tool_input.get(key, "") or ""
            offender = pattern_offender(pat) if pat else None
            if offender:
                deny(
                    f"Blocked: Grep {key} pattern targets a protected secrets file "
                    f"('{offender}'). Restrict the pattern or read .env.example directly."
                )

    elif tool_name == "Bash":
        cmd = tool_input.get("command", "") or ""
        offender = bash_offender(cmd)
        if offender is not None:
            deny(
                f"Blocked: command references '{offender}', a protected secrets file. "
                f"Use the matching .example file "
                f"(e.g. backend/.env.example) to see the schema."
            )

    sys.exit(0)


if __name__ == "__main__":
    main()
