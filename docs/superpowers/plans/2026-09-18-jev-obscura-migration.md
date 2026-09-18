# Jev Obscura Browser Migration Implementation Plan

> **For agentic workers:** REQUIRED: execute with `/luan-coder` (or
> `/old-coder` per task). Each task is one fresh external harness session.
> Follow obra Superpowers `writing-plans` / `subagent-driven-development`.
> Steps use checkbox (`- [ ]`) syntax for tracking.

**GitHub Issue:** #1 — <https://github.com/PauloLuan/jev-obscura-browser/issues/1>

**Goal:** Migrate the browser agent from Browser Use to Obscura Browser (WebSocket CDP), rebrand the package to `jev-obscura-browser`, clean legacy demonstration evidence, and reinitialize git cleanly.

**Architecture:** Replace the `browser-harness` daemon with a native synchronous WebSocket CDP client (`websockets.sync.client`) connecting to Obscura (`ws://127.0.0.1:9222`) with an auto-spawn fallback. Rename `jev_ultrafast` to `jev_obscura_browser`, purge legacy demo media and waitlist docs, update README and branding assets, and reinitialize git against `PauloLuan/jev-obscura-browser`.

**Tech Stack:** Python 3.12+, `websockets`, `httpx`, Obscura (`https://obscura.sh/`), TypeSafe Jev, UV, Pytest, Ruff.

**Spec:** docs/superpowers/specs/2026-09-18-jev-obscura-migration-design.md

**Executor contract:**
- Orchestrator never implements.
- Each open `- [ ]` task is outsourced to one harness (primeagent, opencode,
  grok, agy, codex, claude, …).
- Every implementation prompt starts with `/old-coder` and follows
  SPEC → RED → GREEN → REFACTOR → GAUNTLET → EVIDENCE.
- Mark a checkbox only after EVIDENCE exists on disk.

## Global Constraints

- Never commit secrets or API keys; keep `.env` ignored.
- Tests must pass offline without paid APIs or external network access.
- Code-owned node IDs refer to actual observed elements, never model-generated selectors or arbitrary scripts.
- Never retry a browser mutation.
- Maintain full compatibility with TypeSafe one-request operation/target policy.
- All verification commands must pass: `uv run ruff check .`, `uv run pytest`, `node --check jev_obscura_browser/static/app.js`, `uv build`.

---

### Task 1: Package Renaming & Dependency Updates

**Files:**
- Modify: `pyproject.toml`
- Modify: `.env.example`
- Move: `jev_ultrafast/` -> `jev_obscura_browser/`
- Modify: `uv.lock`

**Interfaces:**
- Consumes: Existing project structure
- Produces: `jev_obscura_browser` package importable with `websockets` installed and `browser-harness` removed.

- [ ] **Step 1: Update `pyproject.toml`**

Replace dependencies and package name:
```toml
[project]
name = "jev-obscura-browser"
version = "0.1.0"
description = "A fast browser agent using Obscura and TypeSafe."
readme = "README.md"
license = "MIT"
requires-python = ">=3.12"
dependencies = [
    "websockets>=13.0,<15",
    "httpx[http2]>=0.28,<1",
]

[project.scripts]
jev-obscura = "jev_obscura_browser.demo:main"
jev = "jev_obscura_browser.demo:main"

[build-system]
requires = ["hatchling"]
build-backend = "hatchling.build"

[dependency-groups]
dev = [
    "pytest>=8.4,<9",
    "ruff>=0.14,<1",
    "pillow>=11,<13",
]

[tool.ruff]
line-length = 120

[tool.ruff.lint]
select = ["E", "F", "I"]

[tool.pytest.ini_options]
testpaths = ["tests"]
```

- [ ] **Step 2: Rename package directory**

```bash
git mv jev_ultrafast jev_obscura_browser
```

- [ ] **Step 3: Update `.env.example`**

Update `.env.example` to include Obscura configuration:
```bash
TYPESAFE_API_KEY=
TEXT_MODEL_API_KEY=
# OpenRouter is the default text helper backend.
TEXT_MODEL_BASE_URL=https://openrouter.ai/api/v1
TEXT_MODEL_NAME=inception/mercury-2.5
TEXT_MODEL_SUPPORTS_REASONING=false
OBSCURA_CDP_URL=ws://127.0.0.1:9222
OBSCURA_BIN=obscura
```

- [ ] **Step 4: Sync dependencies with uv**

Run: `uv sync`
Expected: `uv.lock` updated, `browser-harness` removed, `websockets` installed.

- [ ] **Step 5: Commit changes**

```bash
git add pyproject.toml .env.example jev_obscura_browser uv.lock
git commit -m "chore: rename package to jev-obscura-browser and swap dependencies"
```

---

### Task 2: Native WebSocket CDP Client

**Files:**
- Create: `jev_obscura_browser/cdp.py`
- Create: `tests/test_cdp.py`

**Interfaces:**
- Consumes: `websockets.sync.client`
- Produces: `CDPClient` class and `ensure_obscura()` helper in `jev_obscura_browser.cdp`

- [ ] **Step 1: Write failing unit tests for CDPClient**

Write `tests/test_cdp.py`:
```python
import json
from unittest.mock import MagicMock, patch
import pytest
from jev_obscura_browser.cdp import CDPClient, ensure_obscura

def test_cdp_client_send_and_receive():
    mock_ws = MagicMock()
    mock_ws.recv.return_value = json.dumps({"id": 1, "result": {"targetId": "xyz"}})
    client = CDPClient(ws=mock_ws)
    res = client.call("Target.createTarget", url="about:blank")
    assert res == {"targetId": "xyz"}
    sent = json.loads(mock_ws.send.call_args[0][0])
    assert sent["id"] == 1
    assert sent["method"] == "Target.createTarget"
    assert sent["params"] == {"url": "about:blank"}

def test_cdp_client_handles_session_id():
    mock_ws = MagicMock()
    mock_ws.recv.return_value = json.dumps({"id": 1, "result": {"value": 42}})
    client = CDPClient(ws=mock_ws)
    res = client.call("Runtime.evaluate", session_id="sess-123", expression="1+1")
    assert res == {"value": 42}
    sent = json.loads(mock_ws.send.call_args[0][0])
    assert sent["sessionId"] == "sess-123"

def test_cdp_client_raises_on_error():
    mock_ws = MagicMock()
    mock_ws.recv.return_value = json.dumps({"id": 1, "error": {"message": "Invalid method"}})
    client = CDPClient(ws=mock_ws)
    with pytest.raises(RuntimeError, match="Invalid method"):
        client.call("Bad.method")

def test_ensure_obscura_running_when_probe_succeeds():
    with patch("socket.create_connection"):
        # Does not raise
        ensure_obscura("ws://127.0.0.1:9222")

def test_ensure_obscura_fails_when_unreachable_and_no_binary():
    with patch("socket.create_connection", side_effect=OSError("connection refused")):
        with patch("shutil.which", return_value=None):
            with pytest.raises(RuntimeError, match="Obscura browser is not running"):
                ensure_obscura("ws://127.0.0.1:9222")
```

- [ ] **Step 2: Run test to verify it fails**

Run: `uv run pytest tests/test_cdp.py -v`
Expected: FAIL with `ModuleNotFoundError` or `ImportError`.

- [ ] **Step 3: Implement `jev_obscura_browser/cdp.py`**

Write `jev_obscura_browser/cdp.py`:
```python
"""Direct synchronous WebSocket CDP client for Obscura."""

import json
import os
import shutil
import socket
import subprocess
import time
from urllib.parse import urlparse
from websockets.sync.client import connect

DEFAULT_CDP_URL = "ws://127.0.0.1:9222"

def is_port_open(host: str, port: int, timeout: float = 0.5) -> bool:
    try:
        with socket.create_connection((host, port), timeout=timeout):
            return True
    except OSError:
        return False

def ensure_obscura(cdp_url: str = DEFAULT_CDP_URL) -> None:
    parsed = urlparse(cdp_url)
    host = parsed.hostname or "127.0.0.1"
    port = parsed.port or 9222
    if is_port_open(host, port):
        return

    bin_path = os.getenv("OBSCURA_BIN") or shutil.which("obscura")
    if bin_path:
        subprocess.Popen(
            [bin_path, "serve", "--port", str(port), "--allow-file-access", "--allow-private-network"],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            start_new_session=True,
        )
        deadline = time.monotonic() + 3.0
        while time.monotonic() < deadline:
            if is_port_open(host, port, timeout=0.1):
                return
            time.sleep(0.05)

    raise RuntimeError(
        f"Obscura browser is not running at {cdp_url}.\n"
        "Start it with Docker: docker run -d -p 127.0.0.1:9222:9222 h4ckf0r0day/obscura\n"
        "Or download Obscura from https://obscura.sh/ and run: obscura serve --port 9222"
    )

class CDPClient:
    def __init__(self, cdp_url: str = DEFAULT_CDP_URL, ws=None):
        self.cdp_url = cdp_url
        self._ws = ws
        self._id = 0

    @property
    def ws(self):
        if self._ws is None:
            ensure_obscura(self.cdp_url)
            self._ws = connect(self.cdp_url, max_size=20 * 1024 * 1024)
        return self._ws

    def call(self, method: str, session_id: str | None = None, **params) -> dict:
        self._id += 1
        req_id = self._id
        msg = {"id": req_id, "method": method, "params": params}
        if session_id:
            msg["sessionId"] = session_id
        self.ws.send(json.dumps(msg))

        while True:
            raw = self.ws.recv()
            data = json.loads(raw)
            if data.get("id") == req_id:
                if "error" in data:
                    raise RuntimeError(data["error"].get("message", str(data["error"])))
                return data.get("result", {})

    def close(self):
        if self._ws:
            try:
                self._ws.close()
            except Exception:
                pass
            self._ws = None
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `uv run pytest tests/test_cdp.py -v`
Expected: PASS.

- [ ] **Step 5: Commit changes**

```bash
git add jev_obscura_browser/cdp.py tests/test_cdp.py
git commit -m "feat: add native synchronous WebSocket CDP client for Obscura"
```

---

### Task 3: Browser Engine Integration & Package Re-exports

**Files:**
- Modify: `jev_obscura_browser/browser.py`
- Modify: `jev_obscura_browser/agent.py`
- Modify: `jev_obscura_browser/demo.py`
- Modify: `jev_obscura_browser/model.py`
- Modify: `jev_obscura_browser/questions.py`
- Modify: `tests/test_agent.py`
- Modify: `examples/flights.py`
- Modify: `examples/run.py`

**Interfaces:**
- Consumes: `jev_obscura_browser.cdp.CDPClient`
- Produces: Working `Browser` and `Agent` classes using Obscura CDP without `browser-harness`.

- [ ] **Step 1: Refactor `jev_obscura_browser/browser.py` to use `CDPClient`**

Modify imports and replace `browser_harness` calls with the native `CDPClient`:
```python
"""Observed actions through Obscura Browser; one CDP session, no per-step subprocess."""

import hashlib
import json
import os
import sys
import time
from pathlib import Path

from jev_obscura_browser.cdp import CDPClient, DEFAULT_CDP_URL

READ_STATE = Path(__file__).with_name("snapshot.js").read_text()
MARKER = f"(() => {{ const state={READ_STATE}; return state?.marker ?? null; }})()"

class StalePage(ValueError):
    """A decision no longer refers to the observed page."""

# Module-level default client for mockability in tests
_client = None

def get_client(cdp_url=None):
    global _client
    if _client is None:
        url = cdp_url or os.getenv("OBSCURA_CDP_URL", DEFAULT_CDP_URL)
        _client = CDPClient(url)
    return _client

def cdp(method, session_id=None, **params):
    return get_client().call(method, session_id=session_id, **params)
```

Ensure `Browser.__init__` and `browser_operation` use `cdp(...)` seamlessly.

- [ ] **Step 2: Update all import statements in package and tests**

Change all occurrences of `jev_ultrafast` to `jev_obscura_browser`:
- In `jev_obscura_browser/agent.py`: `from jev_obscura_browser import model` etc.
- In `jev_obscura_browser/demo.py`: `from jev_obscura_browser.agent import Agent` etc.
- In `examples/flights.py`: `from jev_obscura_browser import Agent`
- In `examples/run.py`: `from jev_obscura_browser import Agent`
- In `tests/test_agent.py`: `from jev_obscura_browser import agent as loop`, `from jev_obscura_browser.browser import StalePage...`

- [ ] **Step 3: Run existing unit test suite**

Run: `uv run pytest tests/test_agent.py -v`
Expected: 14 passed.

- [ ] **Step 4: Run ruff lint check**

Run: `uv run ruff check .`
Expected: All checks passed.

- [ ] **Step 5: Commit changes**

```bash
git add jev_obscura_browser/ examples/ tests/
git commit -m "refactor: integrate Obscura CDP client into Browser and Agent"
```

---

### Task 4: Legacy Evidence & Artifact Cleanup

**Files:**
- Delete: `docs/demo.mp4`
- Delete: `docs/demo.gif`
- Delete: `docs/inspector.png`
- Delete: `docs/flights-result.png`
- Delete: `docs/flights-measurement.json`
- Delete: `docs/flights-prepared-measurement.json`
- Delete: `docs/full-speed-measurement.json`
- Delete: `docs/measurement.json`
- Delete: `docs/performance.md`
- Delete: `docs/performance-prepared.md`
- Delete: `docs/launch-draft.md`
- Delete: `scripts/record_flights.py`
- Delete: `scripts/measure_flights.py`
- Delete: `scripts/render_demo.py`

**Interfaces:**
- Consumes: Filesystem
- Produces: Clean docs and scripts directories free of outdated browser-use evidence.

- [ ] **Step 1: Remove legacy media and measurement files**

```bash
rm -f docs/demo.mp4 docs/demo.gif docs/inspector.png docs/flights-result.png
rm -f docs/flights-measurement.json docs/flights-prepared-measurement.json docs/full-speed-measurement.json docs/measurement.json
rm -f docs/performance.md docs/performance-prepared.md docs/launch-draft.md
rm -f scripts/record_flights.py scripts/measure_flights.py scripts/render_demo.py
```

- [ ] **Step 2: Verify deletion**

Run: `ls docs/ scripts/`
Expected: Only `docs/banner.svg` (or new banner), `docs/design.md`, `docs/superpowers/`, and remaining scripts (`check_guards.py`, `render_fixture.py`, `smoke.py`) remain.

- [ ] **Step 3: Commit deletion**

```bash
git add -u
git commit -m "chore: remove legacy browser-use evidence, measurements, and demo videos"
```

---

### Task 5: Documentation, Visual Identity & Web UI Rebranding

**Files:**
- Create: `docs/banner.svg`
- Modify: `README.md`
- Modify: `AGENTS.md`
- Modify: `jev_obscura_browser/static/index.html`

**Interfaces:**
- Consumes: Obscura branding requirements
- Produces: Fully rebranded project documentation and UI.

- [ ] **Step 1: Create fresh SVG banner in `docs/banner.svg`**

Write a dark-mode SVG banner featuring "JEV OBSCURA BROWSER" and "Obscura × TypeSafe" in `docs/banner.svg`.

- [ ] **Step 2: Rewrite `README.md`**

Write clean README:
- Header: `<img src="docs/banner.svg" alt="Jev Obscura Browser · Obscura × TypeSafe" width="100%" />`
- Title: `# Jev Obscura Browser ⚡`
- Description: Fast browser agent powered by Obscura Browser (`https://obscura.sh/`) and TypeSafe Jev.
- Obscura setup instructions:
  - `obscura serve --port 9222` or `docker run -d -p 127.0.0.1:9222:9222 h4ckf0r0day/obscura`
- Quickstart commands:
  - `git clone https://github.com/PauloLuan/jev-obscura-browser.git`
  - `uv sync`
  - `cp .env.example .env`
  - `uv run jev-obscura`
- Library usage example with `jev_obscura_browser`.

- [ ] **Step 3: Update `AGENTS.md`**

Update rules and verification check paths:
```markdown
# Jev Obscura Browser

Read README.md before editing. Keep the loop small: page -> indexed elements -> operation + target -> execution.

- The input is one natural-language goal. Do not add site-specific plans or hardcoded field values.
- TypeSafe chooses an operation and operation-specific target heads in one request. Consume only the selected operation's target.
- Targets must map to observed elements and supported operations. Never let the model emit selectors or executable code.
- TYPE_TEXT invokes the text LLM. Cache a stale retry's value only while its entire helper input is identical.
- Never retry a browser mutation. Log execution before observing its result.
- Screenshots are optional; the model does not consume them. Keep demonstration footage at its original speed.
- Keep credentials server-side and .env ignored. Tests must not call paid APIs.
- Verify actual final outcomes independently. A DONE choice is not proof of success.
- Do not commit or push unless the user requests it.

Checks: uv run ruff check ., uv run pytest, node --check jev_obscura_browser/static/app.js, uv build.
```

- [ ] **Step 4: Update `jev_obscura_browser/static/index.html`**

- Update title to: `<title>Jev Obscura Browser · Obscura × TypeSafe</title>`
- Update brand logo to: `<a href="/" aria-label="Homepage" class="brand">obscura <i>×</i> TypeSafe</a>`
- Update subtitle to remove dead `demo.mp4` link: `Jev picks the next action. A small language model handles the words.`
- Update footer to: `Obscura × TypeSafe · Experimental baseline`

- [ ] **Step 5: Run UI and lint verification**

Run: `node --check jev_obscura_browser/static/app.js`
Expected: 0 syntax errors.
Run: `uv run ruff check .`
Expected: 0 lint errors.

- [ ] **Step 6: Commit changes**

```bash
git add docs/banner.svg README.md AGENTS.md jev_obscura_browser/static/index.html
git commit -m "docs: update branding, documentation, and web UI for Obscura"
```

---

### Task 6: Git Reinitialization & GitHub Remote Verification

**Files:**
- Entire repository `.git/` history

**Interfaces:**
- Consumes: Cleaned workspace
- Produces: Fresh git repository at `main` branch committed and pushed to `PauloLuan/jev-obscura-browser`.

- [ ] **Step 1: Reinitialize git from scratch**

```bash
rm -rf .git .jj
git init -b main
git remote add origin https://github.com/PauloLuan/jev-obscura-browser.git
```

- [ ] **Step 2: Run complete project quality gauntlet**

Run:
```bash
uv run ruff check .
uv run pytest
node --check jev_obscura_browser/static/app.js
uv build
```
Expected: All checks PASS with exit code 0.

- [ ] **Step 3: Create initial commit**

```bash
git add .
git commit -m "feat: initial commit for Jev Obscura Browser"
```

- [ ] **Step 4: Push to GitHub**

```bash
git push -u origin main --force
```
Expected: Successfully pushed to `https://github.com/PauloLuan/jev-obscura-browser`.

- [ ] **Step 5: Re-initialize jj backed by git**

```bash
jj git init
jj status
```
Expected: `Working copy (@) : ... (empty) (no description set)`
Parent commit is the fresh initial commit on `main`.

- [ ] **Step 6: Verify GitHub repository status**

Run: `gh repo view PauloLuan/jev-obscura-browser`
Expected: Clean repository status displaying the updated description and files.
