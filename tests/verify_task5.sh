#!/usr/bin/env bash
set -euo pipefail

echo "=== Running Task 5 Verification Gauntlet ==="

echo "--- 1. Cargo Format Check ---"
cargo fmt --check
echo "PASS: cargo fmt --check"

echo "--- 2. Cargo Check (All Targets) ---"
cargo check --all-targets
echo "PASS: cargo check --all-targets"

echo "--- 3. Cargo Clippy (-D warnings) ---"
cargo clippy --all-targets -- -D warnings
echo "PASS: cargo clippy -- -D warnings"

echo "--- 4. Cargo Test (All Unit & Integration Tests) ---"
cargo test
echo "PASS: cargo test"

echo "--- 5. Static Assets Syntax Check ---"
node --check static/app.js
node --check src/snapshot.js
echo "PASS: node --check"

echo "--- 6. Release Build ---"
cargo build --release
echo "PASS: cargo build --release"

echo "--- 7. Real CLI Execution ---"
./target/release/jev-obscura --help > /dev/null
./target/release/jev --help > /dev/null
./target/release/jev-obscura check
echo "PASS: real CLI execution"

echo "--- 8. Mutation Testing (5/5 Kill Check) ---"
python3 tests/run_task5_mutations.py
echo "PASS: python3 tests/run_task5_mutations.py"

echo "=== Task 5 Verification: ALL GAUNTLET LAYERS PASSED ==="
