# Task 6: Documentation, Visual Identity & Legacy Artifact Purge Specification

**File Path:** `/mnt/paulo_home/home/sources/pauloluan/jev-obscura-browser/docs/superpowers/specs/2026-09-18-task-6-docs-branding-purge-spec.md`  
**Task ID:** Task 6 (#1)  
**Parent Spec:** `/mnt/paulo_home/home/sources/pauloluan/jev-obscura-browser/docs/superpowers/specs/2026-09-18-jev-obscura-migration-design.md`  
**Parent Plan:** `/mnt/paulo_home/home/sources/pauloluan/jev-obscura-browser/docs/superpowers/plans/2026-09-18-jev-obscura-migration.md`  
**Workspace:** `/mnt/paulo_home/home/sources/pauloluan/jev-obscura-browser/.worktrees/task-6`  
**Status:** Approved (100% YOLO Mode)

---

## 1. Objective

Purge all legacy Python media, outdated benchmark artifacts, and obsolete scripts, design a fresh modern SVG dark-mode banner in `docs/banner.svg`, rewrite `README.md` to showcase the pure Rust architecture with Obscura and TypeSafe, update `AGENTS.md` verification checks, and verify all repository quality gates.

---

## 2. Acceptance Criteria & Gherkin Scenarios

### Scenario 1: Fresh Modern SVG Dark-Mode Banner (`docs/banner.svg`)
- **Given** the visual identity requirements for Jev Obscura Browser.
- **When** `docs/banner.svg` is rendered.
- **Then** the SVG root has `viewBox="0 0 1400 480"`, width="1400", and height="480".
- **And** it is well-formed XML parsing without errors via standard XML parsers.
- **And** it features a modern dark-mode aesthetic with dark background (e.g. `#0c0e14` or `#0f1219` with subtle gradients/glow).
- **And** the typography displays "JEV OBSCURA BROWSER" in bold tech font, "Obscura × TypeSafe", and "Fast speculative browser automation in pure Rust".
- **And** visual graphic elements depict a sleek browser window mockup, indexed action elements (`[e1]`, `[e7]`), high-speed decision flow vectors, and neon accents in rust orange (`#ea580c` / `#f97316`) and obscura emerald/cyan (`#10b981` / `#06b6d4`).

### Scenario 2: Legacy Python Media, Benchmarks, and Scripts Purge
- **Given** legacy Python media, benchmark measurements, and obsolete scripts.
- **When** the purge step executes.
- **Then** the following files are completely deleted:
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
- **And** the following directories are completely removed:
  - `scripts/`
  - `examples/`

### Scenario 3: Complete Pure Rust `README.md` Rewrite
- **Given** the pure Rust architecture with Obscura and TypeSafe.
- **When** `README.md` is inspected.
- **Then** it starts with `<img src="docs/banner.svg" alt="Jev Obscura Browser · Obscura × TypeSafe" width="100%" />`.
- **And** title is `# Jev Obscura Browser ⚡ (Rust)`.
- **And** includes badges/status highlights for Pure Rust, Obscura CDP, TypeSafe Jev, and Axum Web Inspector.
- **And** details the Dynamic Indexed Action Space (atomic DOM snapshotting via `src/snapshot.js`, numeric element indices `[1]`, `[2]`, ..., 1-round-trip speculative decision evaluation over operation and target heads, selected target consumption, and flow diagram).
- **And** documents Key Features (zero Python dependencies, async WebSocket CDP, SHA-256 DOM fingerprinting, Axum web inspector on 8766, headless CLI run, Obscura supervisor).
- **And** documents Prerequisites & Obscura Setup (binary `obscura serve --port 9222` and Docker `h4ckf0r0day/obscura`).
- **And** documents Installation & Build (`cargo build --release`).
- **And** documents Configuration (`TYPESAFE_API_KEY`, `TEXT_MODEL_API_KEY`, `OBSCURA_URL`, `OBSCURA_BIN`, `PORT`).
- **And** documents Usage (`cargo run -- serve`, `cargo run -- run`, `cargo run -- check`).
- **And** details Architecture & Safety Rules (mutation safety, freshness checking, model safety).

### Scenario 4: Exact `AGENTS.md` Verification Rules Update
- **Given** the agent development protocol in `AGENTS.md`.
- **When** `AGENTS.md` is updated.
- **Then** its content strictly matches the specified text:
  - Title: `# Jev Obscura Browser (Rust)`
  - Directives on natural-language goal, TypeSafe single-request speculative selection, candidate indexing, TYPE_TEXT helper caching, no mutation retry, screenshots optional, credentials server-side, independent outcome verification, commit/push instructions.
  - Checks: `cargo clippy -- -D warnings, cargo test, node --check static/app.js, cargo build --release.`

### Scenario 5: Repository Quality Gauntlet
- **Given** the updated repository state.
- **When** the gauntlet is executed.
- **Then** `cargo fmt --check` passes with exit code 0.
- **And** `cargo clippy --all-targets -- -D warnings` passes with exit code 0.
- **And** `cargo test` passes with all tests succeeding.
- **And** `node --check static/app.js` passes with 0 syntax errors.
- **And** `cargo build --release` compiles binaries successfully.
- **And** `python3 -c "import xml.etree.ElementTree as ET; ET.parse('docs/banner.svg'); print('SVG OK')"` passes.
- **And** none of the purged files exist.

---

## 3. Setup Plan & Gauntlet Checkpoints

- **VCS Cadence:**
  - Workspace: `/mnt/paulo_home/home/sources/pauloluan/jev-obscura-browser/.worktrees/task-6`
  - Revision Checkpoint: `jj describe -m "docs: rebrand documentation and purge legacy evidence for Rust edition"`
- **Verification Runner:** `tests/verify_task6.sh` automating all layers.
- **Evidence Report:** `/mnt/paulo_home/home/sources/pauloluan/jev-obscura-browser/.luan-coder/20260918-171300/task-6-evidence.md`.
