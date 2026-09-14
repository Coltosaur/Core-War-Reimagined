#!/usr/bin/env python3
"""Tests for the protect-env PreToolUse hook.

Black-box: runs the hook as a subprocess the way Claude Code does, so the
tested surface is the real contract (stdin JSON in, decision JSON out,
exit code) rather than internal functions.

Run: python3 .claude/hooks/test_protect_env.py
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HOOK = Path(__file__).resolve().parent / "protect-env.py"
SETTINGS = Path(__file__).resolve().parent.parent / "settings.json"


def run_hook(payload: str | dict) -> tuple[int, str | None]:
    """Run the hook and return (exit_code, permissionDecision or None)."""
    raw = payload if isinstance(payload, str) else json.dumps(payload)
    proc = subprocess.run(
        [sys.executable, str(HOOK)],
        input=raw,
        capture_output=True,
        text=True,
    )
    decision = None
    if proc.stdout.strip():
        decision = json.loads(proc.stdout)["hookSpecificOutput"]["permissionDecision"]
    return proc.returncode, decision


def tool(name: str, **tool_input) -> dict:
    return {"tool_name": name, "tool_input": tool_input}


class TestBlocksSecrets(unittest.TestCase):
    def assert_denied(self, payload: dict) -> None:
        code, decision = run_hook(payload)
        self.assertEqual(code, 0, "hook should exit 0 and deny via JSON")
        self.assertEqual(decision, "deny", f"expected deny for {payload}")

    def test_read_dotenv(self):
        self.assert_denied(tool("Read", file_path="backend/.env"))

    def test_edit_dotenv_local(self):
        self.assert_denied(tool("Edit", file_path=".env.local"))

    def test_write_dotenv_production(self):
        self.assert_denied(tool("Write", file_path="/abs/path/.env.production"))

    def test_notebook_edit(self):
        self.assert_denied(tool("NotebookEdit", notebook_path="x/.env"))

    def test_grep_path(self):
        self.assert_denied(tool("Grep", path="backend/.env"))

    def test_grep_glob_pattern(self):
        self.assert_denied(tool("Grep", pattern="SECRET", glob=".env*"))

    def test_bash_cat(self):
        self.assert_denied(tool("Bash", command="cat backend/.env"))

    def test_bash_redirect(self):
        self.assert_denied(tool("Bash", command="grep KEY < .env.production"))


class TestAllowsSafeCalls(unittest.TestCase):
    def assert_allowed(self, payload: dict) -> None:
        code, decision = run_hook(payload)
        self.assertEqual(code, 0)
        self.assertIsNone(decision, f"expected no decision (allow) for {payload}")

    def test_example_template_sample_suffixes(self):
        for name in (".env.example", ".env.sample", ".env.template"):
            self.assert_allowed(tool("Read", file_path=f"backend/{name}"))

    def test_unrelated_file(self):
        self.assert_allowed(tool("Read", file_path="frontend/src/api/AuthModal.tsx"))

    def test_lookalike_filenames_are_not_dotenv(self):
        # `my.env-thing` / `foo.envconfig` are not dotenv files.
        self.assert_allowed(tool("Read", file_path="config/my.env-thing"))
        self.assert_allowed(tool("Read", file_path="foo.envconfig"))

    def test_quoted_prose_mentioning_dotenv(self):
        self.assert_allowed(tool("Bash", command='git commit -m "docs: mention .env setup"'))

    def test_unhandled_tool_passes_through(self):
        self.assert_allowed(tool("WebFetch", url="https://example.com"))


class TestFailsClosed(unittest.TestCase):
    """A guard that allows the call when it cannot run is worse than none."""

    def test_malformed_json_denies(self):
        code, decision = run_hook("{not valid json")
        self.assertEqual(code, 0)
        self.assertEqual(decision, "deny")

    def test_empty_stdin_denies(self):
        code, decision = run_hook("")
        self.assertEqual(code, 0)
        self.assertEqual(decision, "deny")

    def test_non_object_payload_denies(self):
        code, decision = run_hook("[1, 2, 3]")
        self.assertEqual(code, 0)
        self.assertEqual(decision, "deny")

    def test_unexpected_error_denies(self):
        # tool_input of the wrong type makes .get() raise inside main().
        code, decision = run_hook('{"tool_name": "Read", "tool_input": "not-a-dict"}')
        self.assertEqual(code, 0)
        self.assertEqual(decision, "deny")


class TestSettingsInvocation(unittest.TestCase):
    """The settings.json command must survive any cwd and fail closed."""

    def command(self) -> str:
        settings = json.loads(SETTINGS.read_text())
        entry = settings["hooks"]["PreToolUse"][0]["hooks"][0]
        return entry["command"]

    def run_command(self, cwd: str, project_dir: str, payload: dict) -> subprocess.CompletedProcess:
        env = dict(os.environ, CLAUDE_PROJECT_DIR=project_dir)
        return subprocess.run(
            ["bash", "-c", self.command()],
            input=json.dumps(payload),
            capture_output=True,
            text=True,
            cwd=cwd,
            env=env,
        )

    def test_matcher_covers_the_guarded_tools(self):
        settings = json.loads(SETTINGS.read_text())
        matcher = settings["hooks"]["PreToolUse"][0]["matcher"]
        for name in ("Read", "Edit", "Write", "NotebookEdit", "Grep", "Bash"):
            self.assertIn(name, matcher)

    def test_resolves_from_a_subdirectory(self):
        # Regression: a relative command path broke whenever the shell cwd
        # moved into a subdirectory, and the hook silently failed open.
        root = str(SETTINGS.resolve().parent.parent)
        sub = os.path.join(root, "frontend")
        if not os.path.isdir(sub):
            self.skipTest("frontend/ not present")
        proc = self.run_command(sub, root, tool("Read", file_path="backend/.env"))
        self.assertNotIn("not found", proc.stderr)
        self.assertEqual(
            json.loads(proc.stdout)["hookSpecificOutput"]["permissionDecision"], "deny"
        )

    def test_missing_hook_script_blocks_with_exit_2(self):
        with tempfile.TemporaryDirectory() as empty:
            proc = self.run_command(empty, empty, tool("Read", file_path="README.md"))
            self.assertEqual(proc.returncode, 2, "missing hook must block, not pass")


if __name__ == "__main__":
    unittest.main(verbosity=2)
