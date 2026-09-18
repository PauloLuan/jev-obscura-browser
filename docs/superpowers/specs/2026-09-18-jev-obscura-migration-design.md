# Jev Obscura Browser (Rust Edition) — Technical Design Specification

**Date:** 2026-09-18  
**Author:** Paulo Luan / Antigravity  
**Status:** Approved  
**Language / Stack:** Rust (Tokio, Axum, Tokio-Tungstenite, Reqwest, Serde)  
**Target Repository:** `PauloLuan/jev-obscura-browser`  

---

## 1. Overview & Architectural Motivation

This specification defines the architectural migration and complete rewrite of the browser agent from Python (`jev-ultrafast` / `browser-use`) to **Rust** (`jev-obscura-browser`), powered natively by **Obscura Browser** (`https://obscura.sh/`).

Obscura is an open-source, high-performance headless browser engine written in Rust that speaks the Chrome DevTools Protocol (CDP) over WebSocket with sub-50ms cold starts, 10× leaner memory than Chrome, and built-in anti-detection. By implementing the agent runtime in Rust:
1. The entire stack aligns natively with Obscura's Rust-first ecosystem.
2. End-to-end decision and action latency is minimized with zero Python runtime overhead.
3. Strong compile-time guarantees enforce state machine correctness and avoid runtime type errors in element selection and CDP dispatch.
4. The deployment artifact becomes a single compiled native binary (`jev-obscura`) with embedded static assets.

---

## 2. Goals & Non-Goals

### Goals
1. **Language Migration:** Completely replace the Python codebase with a idiomatic Rust implementation using Tokio, Serde, Reqwest, and Tokio-Tungstenite.
2. **Obscura CDP Engine:** Implement an asynchronous WebSocket CDP client (`tokio-tungstenite`) communicating directly with `obscura serve` at `ws://127.0.0.1:9222` (configurable via `OBSCURA_CDP_URL`), with auto-spawning of the local `obscura` binary if not already running.
3. **TypeSafe Jev Integration:** Replicate the 1-round-trip speculative operation/target decision protocol (`model.rs`, `questions.rs`) querying TypeSafe's Jev model with strong typed deserialization.
4. **Text Generation Helper:** Call OpenAI-compatible endpoints (OpenRouter `inception/mercury-2.5`, Gemini, etc.) using `reqwest` only when the selected operation is `TYPE_TEXT`.
5. **Inspector Web UI Server:** Implement an `axum` web server serving the interactive inspector UI (`static/index.html`, `static/app.js`, `static/style.css`, `static/fixture.html`) on `http://127.0.0.1:8766` with SSE/REST control endpoints.
6. **Artifact & Legacy Evidence Purge:** Remove all Python files (`jev_ultrafast/`, `pyproject.toml`, `uv.lock`, `.venv`), old demo videos (`demo.mp4`), GIFs (`demo.gif`), screenshots, and outdated benchmark reports from `browser-use`.
7. **Visual Identity & Docs:** Create a fresh SVG banner (`docs/banner.svg`) matching Obscura branding, rewrite `README.md` and `AGENTS.md` for Rust/Cargo.
8. **Testing & Quality Gates:** Provide offline Rust unit and integration tests (`cargo test`) mocking CDP and TypeSafe HTTP responses, enforcing `cargo clippy -- -D warnings` and `cargo build --release`.
9. **Git Reset:** Reinitialize git history from scratch (`git init -b main`) and push a clean initial commit to `https://github.com/PauloLuan/jev-obscura-browser.git`.

### Non-Goals
- Compiling V8 from source in-process (using prebuilt `obscura` binary via WebSocket CDP is chosen for fast <5s compilation).
- Supporting multi-browser backends (Obscura is the primary and only supported engine).
- Calling paid APIs in automated test suites.

---

## 3. System Architecture & Components

```
jev-obscura-browser/
├── Cargo.toml
├── Cargo.lock
├── AGENTS.md
├── LICENSE
├── README.md
├── .env.example
├── docs/
│   ├── banner.svg                   (new Obscura × TypeSafe dark banner)
│   └── superpowers/
│       ├── specs/
│       │   └── 2026-09-18-jev-obscura-migration-design.md
│       └── plans/
│           └── 2026-09-18-jev-obscura-migration.md
├── static/                          (embedded into binary via rust-embed or served from disk)
│   ├── app.js
│   ├── fixture.html
│   ├── index.html
│   └── style.css
├── src/
│   ├── lib.rs                       (public library root)
│   ├── main.rs                      (CLI entrypoint: jev-obscura)
│   ├── agent.rs                     (agent loop: observe -> choose -> act)
│   ├── browser.rs                   (browser high-level abstractions & snapshot evaluator)
│   ├── cdp.rs                       (tokio-tungstenite CDP client & process supervisor)
│   ├── demo.rs                      (axum web server for interactive inspector UI)
│   ├── model.rs                     (TypeSafe API client, choice validation, text LLM)
│   ├── questions.rs                 (TypeSafe dynamic criteria & schema generation)
│   ├── snapshot.js                  (atomic DOM AX snapshot script)
│   └── types.rs                     (serde structs for DOM elements, actions, decisions)
└── tests/
    ├── test_agent.rs                (agent state machine and contract tests)
    └── test_cdp.rs                  (CDP client message routing and error handling tests)
```

---

## 4. Component Specifications

### 4.1 CDP Client (`src/cdp.rs`)
- **Protocol:** JSON-RPC over WebSocket using `tokio-tungstenite`.
- **Concurrency:** Uses an atomic counter for message `id` and a thread-safe map of oneshot response senders `Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, CdpError>>>>>`.
- **Target Sessions:** Passes `sessionId` field when interacting with attached tabs.
- **Process Supervisor (`ensure_obscura`):**
  - Probes `127.0.0.1:9222` via TCP handshake.
  - If unreachable: searches `PATH` and `OBSCURA_BIN` for `obscura`.
  - If found: spawns `obscura serve --port 9222 --allow-file-access --allow-private-network` in detached background mode and waits up to 3.0s for port readiness.
  - If not found: returns a structured error instructing the user to run Docker or download the precompiled binary from `https://obscura.sh/`.

### 4.2 Browser & DOM Perception (`src/browser.rs`)
- High-level browser interface:
  - `Browser::new(url: &str, cdp_url: Option<&str>) -> Result<Self>`
  - `observe(&mut self, screenshot: bool) -> Result<PageState>`
  - `act(&mut self, action: &Action, page: &PageState, text: Option<&str>) -> Result<ActionResult>`
  - `fresh(&self, page: &PageState, action: Option<&Action>) -> Result<bool>`
  - `close(&mut self) -> Result<()>`
- Evaluates `snapshot.js` atomically using `Runtime.evaluate` to return visible elements, bounding boxes, roles, and values.
- Generates a SHA-256 fingerprint from `(url, text, actions, scroll)` to detect page settling.
- Emits `StalePage` errors when DOM elements mutate or shift before an action executes.

### 4.3 TypeSafe Decision Engine & LLM (`src/model.rs`, `src/questions.rs`)
- `ActionSpace`: Partitions observed DOM nodes into candidate actions (`CLICK`, `TYPE_TEXT`, `SELECT`, `SCROLL_UP`, `SCROLL_DOWN`, `WAIT`, `DONE`).
- Submits dynamic criteria to TypeSafe endpoint (`https://api.typesafe.ai/v1/predict` / `/action`) requesting decisions on `operation` and all speculative target heads (`click_target`, `type_text_target`, `select_target`) in **one network round-trip**.
- Only executes the target corresponding to the selected operation.
- If operation is `TYPE_TEXT`: invokes the OpenAI-compatible text endpoint (`TEXT_MODEL_BASE_URL`) with the element context and goal, caching text on identical retries.

### 4.4 Web Inspector Server (`src/demo.rs`)
- Built with `axum` and `tower-http`.
- Serves static assets on `http://127.0.0.1:8766`.
- Endpoints:
  - `GET /`: Serves `index.html`.
  - `POST /api/start`: Starts a new task with goal and URL.
  - `POST /api/step`: Manually steps to the next observation or execution.
  - `POST /api/auto`: Runs continuously until done or blocked.
  - `GET /api/state`: Returns current agent state and latest screenshot.

---

## 5. Cargo Configuration (`Cargo.toml`)

```toml
[package]
name = "jev-obscura-browser"
version = "0.1.0"
edition = "2021"
description = "Fast browser agent in Rust using Obscura and TypeSafe."
license = "MIT"
readme = "README.md"

[[bin]]
name = "jev-obscura"
path = "src/main.rs"

[[bin]]
name = "jev"
path = "src/main.rs"

[dependencies]
tokio = { version = "1.43", features = ["full"] }
tokio-tungstenite = { version = "0.26", features = ["connect"] }
futures-util = "0.3"
reqwest = { version = "0.12", features = ["json"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
axum = "0.8"
tower-http = { version = "0.6", features = ["fs", "cors"] }
clap = { version = "4.5", features = ["derive"] }
tracing = "0.1"
tracing-subscriber = "0.3"
dotenvy = "0.15"
anyhow = "1.0"
thiserror = "2.0"
sha2 = "0.10"

[dev-dependencies]
wiremock = "0.6"
```

---

## 6. Testing & Quality Verification

1. **Unit & Contract Tests (`tests/`):**
   - Mocked CDP frame serialization and deserialization.
   - Offline TypeSafe choice validation tests (`validate_choice`).
   - Agent state machine transitions and stale page error handling.
2. **Linters & Formatters:**
   - `cargo fmt --check`
   - `cargo clippy -- -D warnings`
   - `cargo test`
   - `cargo build --release`
   - `node --check static/app.js`
