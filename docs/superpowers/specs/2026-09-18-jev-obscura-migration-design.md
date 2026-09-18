# Jev Obscura Browser Migration — Technical Design Specification

**Date:** 2026-09-18  
**Author:** Paulo Luan / Antigravity  
**Status:** Approved  
**Target Repository:** `PauloLuan/jev-obscura-browser`  

---

## 1. Overview & Context

This specification defines the architectural migration of the project from **Browser Use** (`browser-use` / `browser-harness`) to **Obscura Browser** (`https://obscura.sh/`), a lightweight, high-performance headless browser engine written in Rust that speaks the Chrome DevTools Protocol (CDP) over WebSocket.

In addition to replacing the browser engine, the project is completely rebranded from `jev-ultrafast` to `jev-obscura-browser`, legacy demonstration assets (videos, gifs, old benchmark measurements) from browser-use are removed, documentation and configuration are updated, and the git history is reinitialized cleanly against `PauloLuan/jev-obscura-browser`.

---

## 2. Goals & Non-Goals

### Goals
1. **Engine Replacement:** Remove `browser-harness` and any `browser-use` dependency. Implement a clean, zero-daemon, native synchronous WebSocket CDP client (`websockets.sync.client`) connecting to `ws://127.0.0.1:9222` (configurable via `OBSCURA_CDP_URL`).
2. **Process Lifecycle:** Provide an `ensure_obscura()` utility that checks for a responsive Obscura server at the configured CDP URL. If unresponsive and the `obscura` binary exists on the host (`PATH` or `OBSCURA_BIN`), auto-spawn `obscura serve --port 9222 --allow-file-access --allow-private-network` in the background. If unavailable, provide actionable guidance (Docker / download command).
3. **Rebranding:** Rename package `jev_ultrafast` to `jev_obscura_browser`, project name in `pyproject.toml` to `jev-obscura-browser`, and CLI command to `jev-obscura` (with alias `jev`).
4. **Legacy Asset Cleanup:** Delete old demonstration footage (`docs/demo.mp4`, `docs/demo.gif`, `docs/inspector.png`, `docs/flights-result.png`), old measurement JSON files (`docs/flights-measurement.json`, etc.), old performance markdown docs referencing the waitlist, and obsolete video rendering scripts.
5. **Visual Identity:** Create a fresh SVG banner (`docs/banner.svg`) reflecting "Jev Obscura Browser · Obscura × TypeSafe".
6. **Documentation & Web UI:** Rewrite `README.md`, `AGENTS.md`, and `jev_obscura_browser/static/index.html` to reflect Obscura and remove all references to Browser Use Cloud / waitlist.
7. **Offline Contracts & TDD:** Preserve the core contract that `pytest` runs offline without requiring paid APIs or live browsers, testing CDP communication through mocks while supporting live runs when Obscura is active.
8. **Git Reinitialization:** Reset git history (`git init`), create initial clean commit, and configure `origin` to `https://github.com/PauloLuan/jev-obscura-browser.git`.

### Non-Goals
- Changing the TypeSafe decision engine or prompt loop (`model.py`, `questions.py`).
- Adding complex multi-browser switching logic (Obscura is the primary and only supported engine).
- Running paid APIs in automated test suites.

---

## 3. Architecture & Component Details

### 3.1 CDP Communication Layer (`jev_obscura_browser/cdp.py`)

Rather than relying on an external IPC daemon (like `browser-harness`), communication with Obscura occurs directly over WebSocket:

- **Protocol:** Chrome DevTools Protocol over WebSocket (default endpoint: `ws://127.0.0.1:9222`).
- **Client Implementation:** Built using `websockets.sync.client.connect`.
  - Maintains persistent WebSocket connection per browser instance.
  - Sends JSON-RPC commands: `{"id": int, "method": str, "params": dict, "sessionId": str (optional)}`.
  - Tracks request/response correlation via monotonically increasing message IDs.
  - Handles session attachment (`Target.attachToTarget` with `flatten: True`) and passes `sessionId` on subsequent scoped calls.
- **Lifecycle Management (`ensure_obscura`):**
  - Attempts socket/HTTP probe to `127.0.0.1:9222`.
  - If unreachable:
    - Checks for `obscura` executable in `PATH` or `OBSCURA_BIN`.
    - If found: spawns `obscura serve --port 9222 --allow-file-access --allow-private-network` in a detached background process and waits up to 3 seconds for port readiness.
    - If not found: raises a descriptive `RuntimeError` instructing the user to start Obscura via Docker (`docker run -d -p 127.0.0.1:9222:9222 h4ckf0r0day/obscura`) or install the binary (`https://obscura.sh/`).

### 3.2 Browser Interface (`jev_obscura_browser/browser.py`)

The `Browser` class retains its public API:
- `Browser(url: str, cdp_url: str | None = None)`
  - Calls `ensure_obscura()`.
  - Creates target tab via `Target.createTarget(url="about:blank")`.
  - Attaches to target via `Target.attachToTarget(targetId=..., flatten=True)`.
  - Configures emulation: `Emulation.setDeviceMetricsOverride(width=1120, height=780, ...)`.
  - Navigates to `url` via `Page.navigate` and polls `document.readyState == "complete"`.
- `observe(screenshot: bool = True) -> dict`
  - Evaluates `snapshot.js` atomically inside the page context.
  - Generates deterministic fingerprint of observed DOM controls and values.
  - Captures JPEG screenshot via `Page.captureScreenshot` when requested.
- `act(action: dict, page: dict, text: str | None = None) -> dict`
  - Validates freshness: element node and guards must match current DOM state.
  - Executes operation:
    - `click`: calculates target center coordinates, dispatches mouse press/release.
    - `fill`: focuses element, selects all, inserts text via `Input.insertText`.
    - `select`: updates dropdown option value and dispatches change events.
    - `scroll`: dispatches mouseWheel event.
    - `wait`: sleeps 100ms.
- `close()`: closes target tab via `Target.closeTarget` and closes WebSocket.

### 3.3 Rebranding & Package Structure

```
jev-obscura-browser/
├── AGENTS.md
├── LICENSE
├── README.md
├── pyproject.toml
├── uv.lock
├── .env.example
├── docs/
│   ├── banner.svg                   (new Obscura × TypeSafe banner)
│   └── superpowers/
│       ├── specs/
│       │   └── 2026-09-18-jev-obscura-migration-design.md
│       └── plans/
│           └── 2026-09-18-jev-obscura-migration.md
├── jev_obscura_browser/
│   ├── __init__.py
│   ├── agent.py
│   ├── browser.py
│   ├── cdp.py                       (new direct CDP client)
│   ├── demo.py
│   ├── model.py
│   ├── questions.py
│   ├── snapshot.js
│   └── static/
│       ├── app.js
│       ├── fixture.html
│       ├── index.html
│       └── style.css
├── examples/
│   ├── flights.py
│   └── run.py
├── scripts/
│   ├── check_guards.py
│   ├── render_fixture.py
│   └── smoke.py
└── tests/
    ├── test_agent.py
    └── test_cdp.py                  (new unit tests for CDP client)
```

### 3.4 Files to Remove

The following legacy files are deleted:
- `docs/demo.mp4`
- `docs/demo.gif`
- `docs/inspector.png`
- `docs/flights-result.png`
- `docs/flights-measurement.json`
- `docs/flights-prepared-measurement.json`
- `docs/full-speed-measurement.json`
- `docs/measurement.json`
- `docs/performance.md`
- `docs/performance-prepared.md`
- `docs/launch-draft.md`
- `scripts/record_flights.py`
- `scripts/measure_flights.py`
- `scripts/render_demo.py`

---

## 4. Dependencies & Environment Configuration

### `pyproject.toml`
- Package name: `jev-obscura-browser`
- Version: `0.1.0`
- Description: `A fast browser agent using Obscura and TypeSafe.`
- Dependencies:
  - `websockets>=13.0,<15`
  - `httpx[http2]>=0.28,<1`
  - *(Drop `browser-harness`)*
- CLI scripts:
  - `jev-obscura = "jev_obscura_browser.demo:main"`
  - `jev = "jev_obscura_browser.demo:main"`

### `.env.example`
```bash
TYPESAFE_API_KEY=
TEXT_MODEL_API_KEY=
TEXT_MODEL_BASE_URL=https://openrouter.ai/api/v1
TEXT_MODEL_NAME=inception/mercury-2.5
TEXT_MODEL_SUPPORTS_REASONING=false
OBSCURA_CDP_URL=ws://127.0.0.1:9222
OBSCURA_BIN=obscura
```

---

## 5. Testing & Quality Gates

1. **Unit Tests (`tests/test_cdp.py`):**
   - Test JSON-RPC serialization and ID routing.
   - Test session scoped requests (`sessionId`).
   - Test timeout handling and error response parsing.
   - Test `ensure_obscura` probe logic with mocks.
2. **Agent Tests (`tests/test_agent.py`):**
   - Update all import paths to `jev_obscura_browser`.
   - Ensure all 14 tests pass without external network or API keys.
3. **Lint & Static Checks:**
   - `uv run ruff check .`
   - `uv run pytest`
   - `node --check jev_obscura_browser/static/app.js`
   - `uv build`
