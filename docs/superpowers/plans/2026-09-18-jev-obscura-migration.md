# Jev Obscura Browser (Rust) Implementation Plan

> **For agentic workers:** REQUIRED: execute with `/luan-coder` (or
> `/old-coder` per task). Each task is one fresh external harness session.
> Follow obra Superpowers `writing-plans` / `subagent-driven-development`.
> Steps use checkbox (`- [ ]`) syntax for tracking.

**GitHub Issue:** #1 — <https://github.com/PauloLuan/jev-obscura-browser/issues/1>

**Goal:** Rewrite the browser agent from Python to Rust, using Obscura Browser over WebSocket CDP, rebranding to `jev-obscura-browser`, purging legacy evidence, and reinitializing git cleanly.

**Architecture:** Implement a high-performance Rust browser agent using Tokio, Serde, and Tokio-Tungstenite to connect to Obscura (`ws://127.0.0.1:9222`) with auto-spawn fallback. Integrate TypeSafe Jev for 1-round-trip speculative decisions, provide an Axum-powered inspector UI server, purge all Python files and legacy browser-use media, and reinitialize git against `PauloLuan/jev-obscura-browser`.

**Tech Stack:** Rust (Tokio 1.43, Tokio-Tungstenite 0.26, Axum 0.8, Reqwest 0.12, Serde 1.0, Clap 4.5), Obscura (`https://obscura.sh/`), TypeSafe Jev.

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
- All verification commands must pass: `cargo clippy -- -D warnings`, `cargo test`, `node --check static/app.js`, `cargo build --release`.

---

### Task 1: Rust Project Scaffolding & Python Purge

**Files:**
- Create: `Cargo.toml`
- Create: `src/lib.rs`
- Modify: `.env.example`
- Move: `jev_ultrafast/static/` -> `static/`
- Move: `jev_ultrafast/snapshot.js` -> `src/snapshot.js`
- Delete: `pyproject.toml`, `uv.lock`, `jev_ultrafast/`

**Interfaces:**
- Consumes: Filesystem
- Produces: Compilable Rust crate root with dependencies configured and static assets placed.

- [x] **Step 1: Write `Cargo.toml`**

Create `Cargo.toml`:
```toml
[package]
name = "jev-obscura-browser"
version = "0.1.0"
edition = "2021"
description = "Fast browser agent in Rust using Obscura and TypeSafe."
license = "MIT"
readme = "README.md"

[lib]
name = "jev_obscura_browser"
path = "src/lib.rs"

[[bin]]
name = "jev-obscura"
path = "src/main.rs"

[[bin]]
name = "jev"
path = "src/bin/jev.rs"

[dependencies]
tokio = { version = "1.43", features = ["full"] }
tokio-tungstenite = { version = "0.26", features = ["connect"] }
futures-util = "0.3"
reqwest = { version = "0.12", features = ["json"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
axum = { version = "0.8", features = ["ws"] }
tower-http = { version = "0.6", features = ["fs", "cors"] }
clap = { version = "4.5", features = ["derive"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
dotenvy = "0.15"
anyhow = "1.0"
thiserror = "2.0"
sha2 = "0.10"
```

- [x] **Step 2: Relocate static assets and snapshot script**

```bash
mkdir -p src static
mv jev_ultrafast/static/* static/
mv jev_ultrafast/snapshot.js src/snapshot.js
```

- [x] **Step 3: Remove legacy Python package and lockfiles**

```bash
rm -rf jev_ultrafast pyproject.toml uv.lock .venv tests/test_agent.py
```

- [x] **Step 4: Create placeholder `src/lib.rs` and `src/main.rs`**

```rust
// src/lib.rs
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
```

```rust
// src/main.rs
fn main() {
    println!("jev-obscura-browser v{}", jev_obscura_browser::VERSION);
}
```

- [x] **Step 5: Verify initial compilation**

Run: `cargo check`
Expected: Download crates and compile cleanly with exit code 0.

- [x] **Step 6: Commit changes**

```bash
git add Cargo.toml Cargo.lock src/ static/ .env.example
git add -u pyproject.toml uv.lock jev_ultrafast/
git commit -m "chore: scaffold Rust project and purge Python files"
```

---

### Task 2: Data Types & TypeSafe Decision Engine in Rust

**Files:**
- Create: `src/types.rs`
- Create: `src/questions.rs`
- Create: `src/model.rs`
- Create: `tests/test_model.rs`

**Interfaces:**
- Consumes: Serde, Reqwest
- Produces: `ActionSpace`, `validate_choice`, `choose`, and `field_text` functions in Rust.

- [x] **Step 1: Write failing unit tests for TypeSafe choice validation and action space**

Write `tests/test_model.rs`:
```rust
use jev_obscura_browser::types::{Action, Choice, PageState};
use jev_obscura_browser::model::{action_space, validate_choice};
use std::collections::HashSet;

#[test]
fn test_invalid_choice_rejected() {
    let mut c = Choice {
        choice: "a".into(),
        confidence: 1.0,
        probabilities: [("a".into(), 1.0), ("b".into(), 0.0)].into_iter().collect(),
    };
    let allowed: HashSet<String> = ["a".into(), "b".into()].into_iter().collect();
    assert!(validate_choice(&c, &allowed).is_ok());

    c.choice = "unseen".into();
    assert!(validate_choice(&c, &allowed).is_err());
}

#[test]
fn test_action_space_partitioning() {
    let actions = vec![
        Action { id: "e1".into(), kind: "fill".into(), label: "Search".into(), role: "textbox".into(), value: "".into(), node: Some(10) },
        Action { id: "e2".into(), kind: "click".into(), label: "Go".into(), role: "button".into(), value: "".into(), node: Some(20) },
    ];
    let (elements, targets, controls) = action_space(&actions);
    assert_eq!(elements.len(), 2);
    assert!(targets.contains_key("TYPE_TEXT"));
    assert!(targets.contains_key("CLICK"));
    assert!(controls.contains(&"WAIT".to_string()));
}
```

- [x] **Step 2: Run test to verify failure**

Run: `cargo test --test test_model`
Expected: FAIL (modules do not exist).

- [x] **Step 3: Implement `src/types.rs`, `src/questions.rs`, and `src/model.rs`**

- Define `Action`, `PageState`, `Choice`, `Decision`, and `TypeSafeRequest` structs with Serde.
- Implement `action_space(actions)` partitioning into operation criteria and target maps.
- Implement `validate_choice(choice, allowed)` ensuring choice exists and probabilities sum to 1.0.
- Implement `choose(page, goal, history)` issuing a single POST to TypeSafe API with speculative heads.
- Implement `field_text(context)` invoking the text LLM endpoint when operation is `TYPE_TEXT`.

- [x] **Step 4: Run tests to verify they pass**

Run: `cargo test --test test_model`
Expected: PASS.

- [x] **Step 5: Commit changes**

```bash
git add src/types.rs src/questions.rs src/model.rs tests/test_model.rs
git commit -m "feat: implement TypeSafe decision model and action space in Rust"
```

---

### Task 3: Asynchronous WebSocket CDP Client & Process Supervisor

**Files:**
- Create: `src/cdp.rs`
- Create: `tests/test_cdp.rs`

**Interfaces:**
- Consumes: `tokio-tungstenite`, `tokio::net::TcpStream`
- Produces: `CdpClient` and `ensure_obscura(cdp_url)` in Rust.

- [ ] **Step 1: Write unit tests for CDP Client frame serialization & supervisor logic**

Write `tests/test_cdp.rs`:
```rust
use jev_obscura_browser::cdp::{format_cdp_request, parse_cdp_response, CdpError};
use serde_json::json;

#[test]
fn test_cdp_request_formatting() {
    let req = format_cdp_request(42, "Target.createTarget", json!({"url": "about:blank"}), None);
    assert_eq!(req["id"], 42);
    assert_eq!(req["method"], "Target.createTarget");
    assert_eq!(req["params"]["url"], "about:blank");
    assert!(req.get("sessionId").is_none());
}

#[test]
fn test_cdp_request_with_session() {
    let req = format_cdp_request(43, "Runtime.evaluate", json!({"expression": "1+1"}), Some("sess-1"));
    assert_eq!(req["sessionId"], "sess-1");
}

#[test]
fn test_cdp_error_parsing() {
    let raw = json!({
        "id": 44,
        "error": {"code": -32000, "message": "Evaluation failed"}
    });
    let result = parse_cdp_response(raw);
    assert!(matches!(result, Err(CdpError::ProtocolError(_))));
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test --test test_cdp`
Expected: FAIL.

- [ ] **Step 3: Implement `src/cdp.rs`**

- Implement `format_cdp_request` and `parse_cdp_response`.
- Implement `CdpClient`: connects via `tokio_tungstenite::connect_async`, launches reader loop routing responses by integer ID to waiting oneshot channels.
- Implement `call(&self, method, params, session_id) -> Result<Value, CdpError>`.
- Implement `ensure_obscura(cdp_url)`: checks TCP connectivity; if offline, checks `OBSCURA_BIN` or `shutil`-equivalent PATH search for `obscura`, auto-spawns `obscura serve`, or returns helpful installation instructions.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --test test_cdp`
Expected: PASS.

- [ ] **Step 5: Commit changes**

```bash
git add src/cdp.rs tests/test_cdp.rs
git commit -m "feat: implement asynchronous WebSocket CDP client and supervisor in Rust"
```

---

### Task 4: Browser Perception & DOM Interaction

**Files:**
- Create: `src/browser.rs`
- Create: `src/agent.rs`
- Create: `tests/test_browser.rs`

**Interfaces:**
- Consumes: `CdpClient`, `snapshot.js`
- Produces: `Browser` and `Agent` runners in Rust.

- [ ] **Step 1: Write integration tests for Browser state & StalePage handling**

Write `tests/test_browser.rs`:
```rust
use jev_obscura_browser::types::PageState;
use jev_obscura_browser::browser::fingerprint;
use serde_json::json;

#[test]
fn test_fingerprint_deterministic() {
    let page1 = PageState {
        url: "https://example.com".into(),
        title: "Example".into(),
        text: "Content".into(),
        scroll: json!({"y": 0}),
        actions: vec![],
        fingerprint: "".into(),
        screenshot: None,
    };
    let fp1 = fingerprint(&page1);
    let fp2 = fingerprint(&page1);
    assert_eq!(fp1, fp2);
    assert_eq!(fp1.len(), 64);
}
```

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test --test test_browser`
Expected: FAIL.

- [ ] **Step 3: Implement `src/browser.rs` and `src/agent.rs`**

- Embed `snapshot.js` via `include_str!("snapshot.js")`.
- Implement `Browser::new(url)` initializing CDP target, viewport metrics, and readyState polling.
- Implement `Browser::observe(screenshot)` running atomic DOM evaluation and returning `PageState`.
- Implement `Browser::act(action, page, text)` verifying target freshness and dispatching input events.
- Implement `Agent::run(goal, max_steps)` running the decision loop `observe -> choose -> act`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --test test_browser`
Expected: PASS.

- [ ] **Step 5: Commit changes**

```bash
git add src/browser.rs src/agent.rs tests/test_browser.rs
git commit -m "feat: implement Browser DOM perception and Agent execution loop in Rust"
```

---

### Task 5: Web Inspector Server & CLI

**Files:**
- Create: `src/demo.rs`
- Create: `src/main.rs`
- Modify: `src/lib.rs`
- Modify: `static/index.html`

**Interfaces:**
- Consumes: `axum`, `tower-http`, `clap`
- Produces: Working CLI executable `jev-obscura` and web inspector on `http://127.0.0.1:8766`.

- [ ] **Step 1: Update `static/index.html`**

Update UI branding:
- Change title to `Jev Obscura Browser · Obscura × TypeSafe`.
- Update brand text to `obscura <i>×</i> TypeSafe`.
- Remove link to `demo.mp4`.
- Update footer to `Obscura × TypeSafe · Experimental baseline`.

- [ ] **Step 2: Implement `src/demo.rs` Axum server**

- Serve static files from `./static` (or embedded in release build).
- Implement REST endpoints:
  - `GET /` -> `index.html`
  - `POST /api/start` -> initializes Agent with given goal and URL.
  - `POST /api/step` -> performs single step.
  - `GET /api/state` -> returns current step, page state, and screenshot.

- [ ] **Step 3: Implement CLI in `src/main.rs`**

- Support subcommands and flags:
  - `jev-obscura serve --port 8766`: runs interactive web inspector.
  - `jev-obscura run --url <URL> --goal <GOAL>`: runs headless in terminal.
  - `jev-obscura check`: health checks Obscura and TypeSafe API connectivity.

- [ ] **Step 4: Verify web asset syntax and Rust compilation**

Run: `node --check static/app.js`
Expected: 0 syntax errors.
Run: `cargo check --bin jev-obscura`
Expected: 0 errors.

- [ ] **Step 5: Commit changes**

```bash
git add src/demo.rs src/main.rs src/lib.rs static/index.html
git commit -m "feat: implement Axum web inspector server and CLI in Rust"
```

---

### Task 6: Documentation, Visual Identity & Legacy Artifact Purge

**Files:**
- Create: `docs/banner.svg`
- Rewrite: `README.md`
- Rewrite: `AGENTS.md`
- Delete: `docs/demo.mp4`, `docs/demo.gif`, `docs/inspector.png`, `docs/flights-result.png`, `docs/flights-measurement.json`, `docs/flights-prepared-measurement.json`, `docs/full-speed-measurement.json`, `docs/measurement.json`, `docs/performance.md`, `docs/performance-prepared.md`, `docs/launch-draft.md`, `scripts/`

**Interfaces:**
- Consumes: Markdown, SVG
- Produces: Clean docs and repository documentation reflecting the Rust implementation.

- [ ] **Step 1: Create fresh SVG banner in `docs/banner.svg`**

Create clean SVG dark-mode banner with "JEV OBSCURA BROWSER" and "Obscura × TypeSafe".

- [ ] **Step 2: Delete legacy media, benchmark files, and old Python scripts**

```bash
rm -f docs/demo.mp4 docs/demo.gif docs/inspector.png docs/flights-result.png
rm -f docs/flights-measurement.json docs/flights-prepared-measurement.json docs/full-speed-measurement.json docs/measurement.json
rm -f docs/performance.md docs/performance-prepared.md docs/launch-draft.md
rm -rf scripts/ examples/
```

- [ ] **Step 3: Rewrite `README.md`**

Rewrite `README.md` with:
- Banner: `<img src="docs/banner.svg" alt="Jev Obscura Browser · Obscura × TypeSafe" width="100%" />`
- Title: `# Jev Obscura Browser ⚡ (Rust)`
- Features: Built in Rust, powered by Obscura Browser (`https://obscura.sh/`) and TypeSafe Jev.
- Installation: `cargo build --release`
- Obscura setup: `obscura serve --port 9222` or `docker run -d -p 127.0.0.1:9222:9222 h4ckf0r0day/obscura`
- Usage: `cargo run -- serve` or `cargo run -- run --url <URL> --goal <GOAL>`.

- [ ] **Step 4: Update `AGENTS.md`**

Update commands for Rust:
```markdown
# Jev Obscura Browser (Rust)

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

Checks: cargo clippy -- -D warnings, cargo test, node --check static/app.js, cargo build --release.
```

- [ ] **Step 5: Commit changes**

```bash
git add docs/banner.svg README.md AGENTS.md
git add -u
git commit -m "docs: rebrand documentation and purge legacy evidence for Rust edition"
```

---

### Task 7: Git Reinitialization & GitHub Remote Verification

**Files:**
- Entire repository git history

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
cargo fmt --check
cargo clippy -- -D warnings
cargo test
node --check static/app.js
cargo build --release
```
Expected: All checks PASS with exit code 0.

- [ ] **Step 3: Create initial commit**

```bash
git add .
git commit -m "feat: initial commit for Jev Obscura Browser (Rust Edition)"
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
Expected: Clean working copy backed by fresh Git `main` commit.

- [ ] **Step 6: Verify GitHub repository status**

Run: `gh repo view PauloLuan/jev-obscura-browser`
Expected: Repository active on GitHub with Rust language detection and clean README.
