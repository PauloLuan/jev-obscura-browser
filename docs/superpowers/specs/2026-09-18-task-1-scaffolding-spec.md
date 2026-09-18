# Task 1: Rust Project Scaffolding & Python Purge Specification

**File Path:** `/mnt/paulo_home/home/sources/pauloluan/jev-obscura-browser/docs/superpowers/specs/2026-09-18-task-1-scaffolding-spec.md`  
**Task ID:** Task 1 (#1)  
**Parent Spec:** `/mnt/paulo_home/home/sources/pauloluan/jev-obscura-browser/docs/superpowers/specs/2026-09-18-jev-obscura-migration-design.md`  
**Parent Plan:** `/mnt/paulo_home/home/sources/pauloluan/jev-obscura-browser/docs/superpowers/plans/2026-09-18-jev-obscura-migration.md`  
**Workspace:** `/mnt/paulo_home/home/sources/pauloluan/jev-obscura-browser/.worktrees/task-1`  
**Status:** Approved (100% YOLO Mode)

---

## 1. Objective

Scaffold the Rust crate for `jev-obscura-browser` with a valid `Cargo.toml`, placeholder `src/lib.rs`, `src/main.rs`, relocated static assets and snapshot script, and complete removal of legacy Python packages and lockfiles. Update `.env.example` to reflect Rust and Obscura runtime requirements.

---

## 2. Acceptance Criteria & Gherkin Scenarios

### Scenario 1: Rust Crate Scaffolding & Dependency Resolution
- **Given** `Cargo.toml` at workspace root configured with package `jev-obscura-browser` (v0.1.0, edition 2021), lib `jev_obscura_browser` (`src/lib.rs`), and binaries `jev-obscura` (`src/main.rs`) and `jev` (`src/bin/jev.rs`).
- **When** executing `cargo check`, `cargo check --bin jev-obscura`, `cargo check --bin jev`, and `cargo test`.
- **Then** cargo successfully parses `Cargo.toml`, resolves all dependencies, and compiles without warnings or errors (exit code 0).

### Scenario 2: Static Asset & Script Relocation
- **Given** legacy assets in `jev_ultrafast/static/` and script `jev_ultrafast/snapshot.js`.
- **When** relocated to `static/` (`app.js`, `fixture.html`, `index.html`, `style.css`) and `src/snapshot.js`.
- **Then** files exist at target locations with byte-identical checksums compared to original.
- **And** executing `node --check static/app.js` and `node --check src/snapshot.js` exits with 0.

### Scenario 3: Complete Python Package Purge
- **Given** legacy Python artifacts: `jev_ultrafast/`, `pyproject.toml`, `uv.lock`, `.venv`, `tests/test_agent.py`.
- **When** purged from the repository.
- **Then** none of these files or directories exist.
- **And** `find . -name "*.py"` returns no legacy agent files.

### Scenario 4: Environment Variable Specification
- **Given** `.env.example`.
- **When** updated for the Rust/Obscura architecture.
- **Then** it contains keys for Obscura (`OBSCURA_CDP_URL`, `OBSCURA_BIN`), TypeSafe (`TYPESAFE_API_KEY`, `TYPESAFE_MODEL`), Text LLM (`TEXT_MODEL_API_KEY`, `TEXT_MODEL_BASE_URL`, `TEXT_MODEL`, `TEXT_MODEL_REASONING`), and logging (`RUST_LOG`).

---

## 3. Negative Constraints (What Must NOT Happen)

1. **No Data Loss on Assets:** The contents of `app.js`, `fixture.html`, `index.html`, `style.css`, and `snapshot.js` must remain intact and valid.
2. **No Paid API Calls:** No tests or checks may reach out to paid APIs or live LLM endpoints.
3. **No Direct Git Commands:** All version control tracking and checkpoints must be executed via `jj`.
4. **No Dangling References:** Cargo configuration must not reference non-existent files or invalid crate names.

---

## 4. Dependencies & Justification

| Dependency | Version | Justification |
|---|---|---|
| `tokio` | 1.43 | Asynchronous runtime for async I/O, timers, and concurrency. |
| `tokio-tungstenite` | 0.26 | WebSocket client for direct communication with Obscura CDP. |
| `futures-util` | 0.3 | Stream/sink combinators for WebSocket handling. |
| `reqwest` | 0.12 | HTTP client for calling TypeSafe and OpenAI-compatible text endpoints. |
| `serde` | 1.0 | Core serialization/deserialization framework. |
| `serde_json` | 1.0 | JSON serialization/deserialization for CDP frames and API payloads. |
| `axum` | 0.8 | Lightweight, robust web framework for the web inspector UI server. |
| `tower-http` | 0.6 | Static file serving (`fs`) and CORS middleware for Axum. |
| `clap` | 4.5 | Command-line argument parsing with derive macros. |
| `tracing` | 0.1 | Structured diagnostic and event logging instrumentation. |
| `tracing-subscriber` | 0.3 | Formatting and env-filter support for tracing events. |
| `dotenvy` | 0.15 | Zero-overhead `.env` file loader for Rust. |
| `anyhow` | 1.0 | Idiomatic application-level error management. |
| `thiserror` | 2.0 | Derive macro for domain error enums. |
| `sha2` | 0.10 | SHA-256 implementation for deterministic page state fingerprinting. |
| `wiremock` (dev) | 0.6 | Offline HTTP server mocking for TypeSafe API contract tests. |

---

## 5. Verification Plan

1. **Cargo checks:**
   - `cargo check --all-targets`
   - `cargo test`
2. **Node syntax checks:**
   - `node --check static/app.js`
   - `node --check src/snapshot.js`
3. **Filesystem audit:**
   - Verify non-existence of `jev_ultrafast/`, `pyproject.toml`, `uv.lock`, `tests/test_agent.py`.
   - Verify existence of `Cargo.toml`, `src/lib.rs`, `src/main.rs`, `src/bin/jev.rs`, `src/snapshot.js`, `static/app.js`, `static/index.html`.

---

## 6. Revisions & Corrections

### Revision Round 1 (2026-09-18)
- **Finding:** Pointing multiple `[[bin]]` targets (`jev-obscura` and `jev`) to the single file `src/main.rs` caused `cargo check --all-targets` and `cargo test` to emit compiler warning `file src/main.rs found to be present in multiple build targets`.
- **Correction:** Separated binary paths: `jev-obscura` targets `src/main.rs` and `jev` targets `src/bin/jev.rs`. Both binaries execute cleanly with ZERO warnings and ZERO errors.
