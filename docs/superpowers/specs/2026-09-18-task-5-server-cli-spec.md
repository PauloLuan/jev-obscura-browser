# Task 5: Web Inspector Server & CLI in Rust Specification

**File Path:** `/mnt/paulo_home/home/sources/pauloluan/jev-obscura-browser/docs/superpowers/specs/2026-09-18-task-5-server-cli-spec.md`  
**Task ID:** Task 5 (#1)  
**Parent Spec:** `/mnt/paulo_home/home/sources/pauloluan/jev-obscura-browser/docs/superpowers/specs/2026-09-18-jev-obscura-migration-design.md`  
**Parent Plan:** `/mnt/paulo_home/home/sources/pauloluan/jev-obscura-browser/docs/superpowers/plans/2026-09-18-jev-obscura-migration.md`  
**Workspace:** `/mnt/paulo_home/home/sources/pauloluan/jev-obscura-browser/.worktrees/task-5`  
**Status:** Approved (100% YOLO Mode)

---

## 1. Objective

Implement `src/demo.rs` (Axum web inspector server, shared thread-safe `AppState`, REST API endpoints, static asset serving), `src/main.rs` & `src/bin/jev.rs` (Clap CLI with `serve`, `run`, `check` subcommands), export `pub mod demo;` in `src/lib.rs`, update branding and links in `static/index.html`, and prove functionality with comprehensive offline tests in `tests/test_server.rs`.

---

## 2. Acceptance Criteria & Gherkin Scenarios

### Scenario 1: Web Inspector Index Route (`GET /`)
- **Given** an Axum router constructed via `demo::create_router(state)`.
- **When** a `GET /` request is processed via `oneshot`.
- **Then** the response status is `200 OK`.
- **And** the `Content-Type` header contains `text/html`.
- **And** the body contains `Jev Obscura Browser` and `obscura <i>×</i> TypeSafe`.

### Scenario 2: Initial Clean State (`GET /api/state`)
- **Given** a freshly initialized `AppState` behind `Arc<RwLock<AppState>>`.
- **When** a `GET /api/state` request is dispatched.
- **Then** the response status is `200 OK`.
- **And** the JSON response has `"status": "idle"`.
- **And** `"goal"` is empty, `"page"` is null, `"decision"` is null, and `"history"` is an empty list.

### Scenario 3: Agent Task Start and Reset (`POST /api/start` and `POST /api/reset`)
- **Given** an initialized router.
- **When** a `POST /api/start` request is sent with payload `{"goal": "Find flights from Zurich to London", "scenario": "flights"}`.
- **Then** the response status is `200 OK`.
- **And** the response JSON reflects `"goal": "Find flights from Zurich to London"`, `"scenario": "flights"`, `"status": "ready"`.
- **And** a subsequent `POST /api/reset` with a new goal resets history and decisions and updates the goal.

### Scenario 4: Step Advancement (`POST /api/step`)
- **Given** a router with initialized agent state.
- **When** a `POST /api/step` request is sent.
- **Then** the response status is `200 OK`.
- **And** the agent state advances, recording an executed decision into `history` and updating `decision`.

### Scenario 5: Interactive Prediction and Action Endpoints (`POST /api/predict`, `POST /api/act`, `POST /api/tick`)
- **Given** a router with active state.
- **When** `POST /api/predict` is called.
- **Then** the status changes to `"predicted"` and `decision` is set.
- **When** `POST /api/act` is called.
- **Then** the predicted decision is committed to `history` and status returns to `"ready"`.
- **When** `POST /api/tick` is called.
- **Then** the endpoint returns `200 OK` with the current state JSON.

### Scenario 6: CLI Argument Parsing (`serve`, `run`, `check`)
- **Given** the Clap CLI definition in `demo::Cli`.
- **When** parsing arguments `["jev-obscura", "serve", "--port", "9000"]`.
- **Then** it produces `Commands::Serve { port: 9000 }`.
- **When** parsing arguments `["jev-obscura", "run", "--url", "https://example.com", "--goal", "Click button", "--max-steps", "15"]`.
- **Then** it produces `Commands::Run { url: "https://example.com", goal: "Click button", max_steps: 15 }`.
- **When** parsing arguments `["jev-obscura", "check"]`.
- **Then** it produces `Commands::Check`.

### Scenario 7: Static Asset UI & Branding Updates
- **Given** `static/index.html`.
- **When** inspected.
- **Then**:
  1. `<title>` contains `Jev Obscura Browser · Obscura × TypeSafe`.
  2. Brand link contains `obscura <i>×</i> TypeSafe`.
  3. The link to `demo.mp4` is removed.
  4. The footer contains `Obscura × TypeSafe · Experimental baseline`.

---

## 3. Negative Constraints

1. **Zero External Network in Tests:** `tests/test_server.rs` must execute 100% offline via in-memory `oneshot` requests without binding physical network ports or requiring a running browser.
2. **Zero Direct Git Commands:** All version control checkpoints and workspace operations use `jj`.
3. **No Unhandled Panics on Missing Browser:** Endpoints like `/api/step` or `/api/start` must handle `browser == None` gracefully without crashing the server.
4. **Clean Toolchain Output:** All code must pass `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `cargo fmt --check`, and `node --check static/app.js`.

---

## 4. Setup Plan & Dependencies

1. Add `tower = { version = "0.5", features = ["util"] }` to `[dev-dependencies]` in `Cargo.toml`.
   - *Justification:* Required for `tower::ServiceExt::oneshot` to test Axum routers in-memory. (Already present in `Cargo.lock` transitively).
2. Use `jj new` / `jj describe` checkpoints during development.

---

## 5. Architecture & Module Design

### 5.1 `src/demo.rs`
- `AppState`:
  - `status: String` ("idle", "ready", "predicted", "done", "blocked")
  - `goal: String`
  - `url: Option<String>`
  - `scenario: Option<String>`
  - `page: Option<PageState>`
  - `elements: Vec<serde_json::Value>`
  - `decision: Option<Decision>`
  - `decisions: Vec<Decision>`
  - `history: Vec<Decision>`
  - `screenshot: Option<String>`
  - `text_model: String`
  - `plan: Vec<String>`
  - `plan_index: usize`
  - `elapsed_ms: u64`
  - `max_steps: usize`
  - `browser: Option<Browser>` (skipped from serialization)
  - `config: AgentConfig` (skipped from serialization)
- `StateResponse`: Serialized representation of `AppState` for REST consumers.
- `create_router(state: Arc<tokio::sync::RwLock<AppState>>) -> axum::Router`
- `serve(port: u16) -> Result<(), anyhow::Error>`
- `Cli` and `Commands` enum deriving `clap::Parser` and `clap::Subcommand`.
- `run_cli() -> anyhow::Result<()>`

### 5.2 `src/main.rs` & `src/bin/jev.rs`
- Thin entrypoint delegating to `jev_obscura_browser::demo::run_cli().await`.

### 5.3 `src/lib.rs`
- Add `pub mod demo;`.

---

## 6. Verification Gauntlet Checklist

- [ ] `cargo check --all-targets`
- [ ] `cargo clippy --all-targets -- -D warnings`
- [ ] `cargo test`
- [ ] `cargo fmt --check`
- [ ] `node --check static/app.js && node --check src/snapshot.js`
- [ ] Manual mutation testing on `src/demo.rs`
- [ ] Real execution test of CLI `--help` and `check`
