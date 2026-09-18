# Superpowers Drain Queue — Run 20260918-160949 (resume; agy credits exhausted)

## Plan
- Path: `docs/superpowers/plans/2026-09-18-jev-obscura-migration.md`
- Spec: `docs/superpowers/specs/2026-09-18-jev-obscura-migration-design.md`
- Issue: #1 — https://github.com/PauloLuan/jev-obscura-browser/issues/1
- Bind verified: `gh issue view 1` body lines 80-83 name this plan path. No invented numbers.

## Tasks
1. Task 1: Scaffolding & Python Purge [COMPLETED, evidence+critique in run 20260918-171300]
2. Task 2: TypeSafe Decision Engine [COMPLETED, evidence+critique in run 20260918-171300]
3. Task 3: CDP Client & Supervisor [COMPLETED, evidence+critique in run 20260918-171300]
4. Task 4: Browser Perception & Agent [COMPLETED, evidence+critique in run 20260918-171300]
5. Task 5: Inspector Server & CLI [COMPLETED, evidence+critique in run 20260918-171300]
6. Task 6: Docs, Branding & Purge [COMPLETED in plan + commit e6e4fa09, BUT no task-6-evidence.md/critique on disk → EVIDENCE GAP, folded into Task 7 Step 3]
7. Task 7: Safe Landing (AMENDED 2026-09-18 per user: rebase + gauntlet + push without force; destructive reinit rejected) [PENDING dispatch]

## Repo state at dispatch
- Local `main` == `origin/main` == `1231850` (Cloud-waitlist + Python era). Zero divergence.
- Tasks 1-6 commits unpushed at jj `@ 9450998d`.
- Uncommitted: `M .gitignore` (adds `.worktrees/`).
- No `task-7-*` spec exists; builder uses design spec as context + amended steps as acceptance.

## Harness map
- Builder: `primeagent` (fitness `recommend ops-infra×builder` → explore n<3).
- Critic: `dsh` (fitness `recommend ops-infra×critic` → explore n<3). Must differ from builder. Satisfied.
- Excluded: `agy` exhausted 4h (user-reported credits out, 2026-09-18); `claude`,`codex`,`grok`,`opencode` exhausted per ledger.
- Launch: Herdr new tab in workspace `w1Z`, `herdr pane run` (primeagent/dsh are not Herdr kinds).
