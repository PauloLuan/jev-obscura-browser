/old-coder
================================================================================
MISSION: Task 7 — Safe Landing: Rebase, Full Gauntlet & Push Without Force
HARNESS: pi --provider google --model google/gemini-2.5-flash (verified working; do NOT use openai-codex — its OAuth is expired)
TASK CLASS: ops-infra
================================================================================

PLAN: /mnt/paulo_home/home/sources/pauloluan/jev-obscura-browser/docs/superpowers/plans/2026-09-18-jev-obscura-migration.md
ISSUE: #1 https://github.com/PauloLuan/jev-obscura-browser/issues/1
SPEC: /mnt/paulo_home/home/sources/pauloluan/jev-obscura-browser/docs/superpowers/specs/2026-09-18-jev-obscura-migration-design.md
EVIDENCE: /mnt/paulo_home/home/sources/pauloluan/jev-obscura-browser/.luan-coder/20260918-160949/task-7-evidence.md

CONTEXT & OBJECTIVE:
Jev Obscura Browser Rust migration. Tasks 1-6 are done (Rust crate, TypeSafe
decision engine, async CDP client, Browser/Agent loop, Axum inspector + CLI,
docs rebrand). That work sits UNPUSHED at jj `@ 9450998d`. Local `main` ==
`origin/main` == `1231850` (old Python-era history incl. the Cloud-waitlist
commit). Execute the AMENDED Task 7 Steps 1-6 in PLAN exactly (the amendment
note at the top of Task 7 overrides the old destructive steps): preserve
state, rebase Rust commits onto main, run the full quality gauntlet, re-verify
Task 6 outputs (its EVIDENCE file is missing — this closes that gap), move the
`main` bookmark fast-forward, push WITHOUT force, confirm clean colocated
state, verify GitHub.

HARD NEGATIVE CONSTRAINTS (violating any of these fails the task):
- NEVER `rm -rf .git` or `.jj`. NEVER `git push --force` (or any force/lease
  override). NEVER rewrite pushed history: `1231850` and its ancestors are
  immutable; only the unpushed Rust commits may be rebased onto `main`.
- Work in the default checkout
  `/mnt/paulo_home/home/sources/pauloluan/jev-obscura-browser` (this task moves
  shared refs; a jj workspace cannot isolate ref moves — record this reason).
- All VCS via `jj` (`jj st/commit/rebase/bookmark/log/status`); `gh` only for
  GitHub reads. Never `git checkout/branch/rebase` when jj covers it.
- Keep `.env` ignored; no secrets in commits or EVIDENCE. Tests stay offline-safe.
- Commit after each finished step slice with `jj commit -m "<plan message>"`.

SPECIFICATION REQUIREMENTS:
1. This is an ops task: acceptance = amended Steps 1-6 + the gauntlet table
   below. Put a short SPEC header (behaviors + setup actually used + no new
   dependencies) at the top of EVIDENCE. The SPEC path above is context.
2. Follow SPEC → GAUNTLET → EVIDENCE. No product code changes expected. If a
   gate fails: capture the failing output FIRST (RED), then fix minimally
   (GREEN), then re-run the full gauntlet. Never weaken a gate, never
   `--no-verify`, never skip a failing layer silently.
3. If the remote moved under you: `jj git fetch` + rebase + retry the push.
   Still never force.

GAUNTLET LAYERS (every command must exit 0; paste exit codes + stdout tails):
- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test`
- `node --check static/app.js`
- `cargo build --release`
- Task 6 re-verification (Step 3): `docs/banner.svg` present/branded,
  `README.md` Rust rebrand, `AGENTS.md` Rust checks, legacy media/benchmarks/
  `scripts/`/`examples/` absent — with `ls`/`grep` proofs.
- History-preservation proof: `git merge-base --is-ancestor 1231850 main`
  (must succeed) + `jj log --limit 12`.
- Landing proof: `jj status` clean, `jj bookmark list` (main tracks
  origin/main), `git log origin/main --oneline -8`, `gh repo view
  PauloLuan/jev-obscura-browser`.

OUTPUT EVIDENCE:
Save the complete reproducible report to the EVIDENCE path: per-step command,
exit code, stdout tail; preservation proof; push result (fast-forward,
no force); Task 6 findings; anything failed and how resolved; known limits.

Do NOT mark plan checkboxes. The orchestrator flips them after independent
verification. Do NOT open or close the GitHub issue.
