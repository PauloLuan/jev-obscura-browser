# Task 4: Browser Perception & DOM Interaction in Rust Specification

**File Path:** `/mnt/paulo_home/home/sources/pauloluan/jev-obscura-browser/docs/superpowers/specs/2026-09-18-task-4-browser-agent-spec.md`  
**Task ID:** Task 4 (#1)  
**Parent Spec:** `/mnt/paulo_home/home/sources/pauloluan/jev-obscura-browser/docs/superpowers/specs/2026-09-18-jev-obscura-migration-design.md`  
**Parent Plan:** `/mnt/paulo_home/home/sources/pauloluan/jev-obscura-browser/docs/superpowers/plans/2026-09-18-jev-obscura-migration.md`  
**Workspace:** `/mnt/paulo_home/home/sources/pauloluan/jev-obscura-browser/.worktrees/task-4`  
**Status:** Approved (100% YOLO Mode)

---

## 1. Objective

Implement `src/browser.rs` (high-level CDP browser perception, DOM snapshotting, deterministic fingerprinting, element freshness guarding, action dispatching) and `src/agent.rs` (autonomous decision execution loop: observe -> choose -> [field_text] -> act), export both in `src/lib.rs`, and prove correctness with a comprehensive, offline test suite in `tests/test_browser.rs`.

---

## 2. Acceptance Criteria & Gherkin Scenarios

### Scenario 1: Deterministic Page State Fingerprinting
- **Given** a `PageState` instance with specific `url`, `text`, `actions`, and `scroll` coordinates.
- **When** `fingerprint(&page)` is calculated.
- **Then** it produces a deterministic 64-character SHA-256 lowercase hex string.
- **And** calling `fingerprint(&page)` repeatedly yields identical results.

### Scenario 2: Mutation Detection via Fingerprint Diff
- **Given** a base `PageState` and its fingerprint.
- **When** any constituent attribute (`url`, `text`, `actions`, `scroll`) is mutated.
- **Then** the resulting `fingerprint` is strictly different from the base fingerprint.

### Scenario 3: Browser Session Initialization & Target Navigation
- **Given** an active `CdpClient` connected to a WebSocket CDP endpoint.
- **When** `browser.open(url).await` is called.
- **Then** the browser creates a new target via `Target.createTarget`, attaches with `Target.attachToTarget`, configures viewport emulation (`Emulation.setDeviceMetricsOverride`), enables focus (`Emulation.setFocusEmulationEnabled`), dispatches `Page.navigate`, and awaits `document.readyState == "complete"`.

### Scenario 4: DOM Perception via `snapshot.js`
- **Given** an open browser tab.
- **When** `browser.observe(screenshot: false).await` is called.
- **Then** it executes `snapshot.js` atomically using `Runtime.evaluate`, parses elements into `PageState`, computes and assigns `fingerprint`, and returns the observed `PageState`.
- **And** if the page is settling or throws `StalePage`, it retries up to 10 attempts before failing.

### Scenario 5: Optional Screenshot Capture
- **Given** `browser.observe(screenshot: true).await`.
- **When** the DOM snapshot evaluation succeeds.
- **Then** it executes `Page.captureScreenshot` with `format: "jpeg"` and `quality: 72`, populating `page.screenshot` with the base64 JPEG payload.

### Scenario 6: Target Element Freshness Checking
- **Given** an observed `PageState` and candidate `Action`.
- **When** `browser.fresh(&page, Some(&action)).await` is evaluated.
- **Then** it evaluates the node guard and page key against `window.__jevFast` in the page context, returning `true` if unchanged or `false` if mutated/stale.

### Scenario 7: StalePage Rejection During `act`
- **Given** an action targeting an element that is no longer fresh.
- **When** `browser.act(&action, &page, text).await` is invoked.
- **Then** it returns `Err(BrowserError::StalePage(_))` and avoids dispatching any click or keyboard events.

### Scenario 8: Interactive Input Execution (`click`, `fill`, `select`)
- **Given** a fresh interactive action (`click` or `fill` or `select`).
- **When** `browser.act(&action, &page, text).await` executes.
- **Then**:
  1. For `click`: evaluates node coordinates and dispatches `mousePressed` and `mouseReleased`.
  2. For `fill`: evaluates node coordinates, clicks, dispatches select-all key combination, and calls `Input.insertText`.
  3. For `select`: updates the `<select>` value and dispatches `input` and `change` DOM events.
  4. Stores `action` in `browser.after_input` to enable stabilization during next observation.

### Scenario 9: Control Actions (`scroll`, `wait`)
- **Given** an action with `kind == "scroll"`.
- **When** `browser.act` executes.
- **Then** it dispatches `Input.dispatchMouseEvent` with `mouseWheel` and `deltaY`.
- **Given** an action with `kind == "wait"`.
- **When** `browser.act` executes.
- **Then** it sleeps briefly (e.g. 100ms) without dispatching mouse/keyboard events.

### Scenario 10: Agent Decision Execution Step (`Agent::step`)
- **Given** an initialized `Browser` and `AgentConfig`.
- **When** `Agent::step(&mut browser, goal, &mut history, &config).await` executes.
- **Then**:
  1. Observes `page = browser.observe(true)`.
  2. Calls `model::choose` with action space and questions.
  3. If operation is `TYPE_TEXT` and `decision.text` is None, calls `model::field_text` to generate text.
  4. If operation is `DONE`, appends decision to `history` and returns `Ok(Some(decision))` immediately.
  5. Dispatches `browser.act` for the chosen target.
  6. Appends executed decision to `history` and returns `Ok(Some(decision))`.

### Scenario 11: Agent Run Loop Termination & Step Budget
- **Given** a goal where `step` produces `DONE`.
- **When** `Agent::run(&mut browser, goal, &config).await` executes.
- **Then** it completes cleanly returning `Ok(history)`.
- **And Given** an execution exceeding `config.max_steps` without `DONE`.
- **Then** it returns `Err(AgentError::MaxStepsExceeded)`.

---

## 3. Negative Constraints

1. **Zero External Network Calls:** 100% of integration tests must run against local loopback listeners (`TcpListener::bind("127.0.0.1:0")` and `wiremock::MockServer`).
2. **Zero Direct Git Commands:** All version control checkpoints and workspace operations use `jj`.
3. **No Retries on Dispatched Inputs:** User interactive actions are dispatched strictly once per step; freshness gates precede input dispatch.
4. **Deterministic Reproducibility:** Fingerprinting is invariant across runs and platforms.
5. **Zero Warnings & Zero Lint Errors:** Code must compile cleanly under `cargo clippy --all-targets -- -D warnings`.

---

## 4. Architecture & Module Design

### 4.1 `src/browser.rs`
- Embed `snapshot.js` via `include_str!("snapshot.js")`.
- `fingerprint(page: &PageState) -> String`
- `BrowserError`:
  - `StalePage(String)`
  - `TargetNotFound(String)`
  - `Cdp(#[from] crate::cdp::CdpError)`
  - `Json(#[from] serde_json::Error)`
  - `ExecutionError(String)`
- `Browser`:
  - `client: Arc<CdpClient>`
  - `target_id: Option<String>`
  - `session_id: Option<String>`
  - `after_input: Option<Action>`
  - Methods: `new`, `open`, `observe`, `fresh`, `act`, `close`.

### 4.2 `src/agent.rs`
- `AgentError`:
  - `Browser(#[from] crate::browser::BrowserError)`
  - `Model(#[from] crate::model::ModelError)`
  - `MaxStepsExceeded`
  - `Other(String)`
- `AgentConfig`:
  - `max_steps: usize`
  - `typesafe_base_url: Option<String>`
  - `typesafe_api_key: Option<String>`
  - `typesafe_model: Option<String>`
  - `text_model_base_url: Option<String>`
  - `text_model_api_key: Option<String>`
  - `text_model: Option<String>`
- `Agent`:
  - `step(...) -> Result<Option<Decision>, AgentError>`
  - `run(...) -> Result<Vec<Decision>, AgentError>`

### 4.3 Data Types (`src/types.rs`)
- Add `pub fingerprint: String` to `PageState` with `#[serde(default)]`.
- Add `pub marker: Option<Value>`, `pub page_key: Option<Value>`, `pub guards: Option<Value>` with `#[serde(default, skip_serializing_if = "Option::is_none")]`.

---

## 5. Verification Gauntlet Checklist

- [ ] `cargo check --all-targets`
- [ ] `cargo clippy --all-targets -- -D warnings`
- [ ] `cargo test`
- [ ] `cargo fmt --check`
- [ ] `node --check static/app.js && node --check src/snapshot.js`
- [ ] Mutation testing on `src/browser.rs` and `src/agent.rs`
