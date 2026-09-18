#!/usr/bin/env bash
set -euo pipefail

echo "=== Running Task 1 Scaffolding Verification ==="

FAILED=0

# Check 1: Cargo.toml and Rust sources
if [[ ! -f "Cargo.toml" ]]; then
    echo "FAIL: Cargo.toml not found"
    FAILED=1
else
    echo "PASS: Cargo.toml exists"
fi

if [[ ! -f "src/lib.rs" ]]; then
    echo "FAIL: src/lib.rs not found"
    FAILED=1
else
    echo "PASS: src/lib.rs exists"
fi

if [[ ! -f "src/main.rs" ]]; then
    echo "FAIL: src/main.rs not found"
    FAILED=1
else
    echo "PASS: src/main.rs exists"
fi

if [[ ! -f "src/bin/jev.rs" ]]; then
    echo "FAIL: src/bin/jev.rs not found"
    FAILED=1
else
    echo "PASS: src/bin/jev.rs exists"
fi

# Check 2: Relocated static assets and snapshot script
if [[ ! -f "src/snapshot.js" ]]; then
    echo "FAIL: src/snapshot.js not found"
    FAILED=1
else
    echo "PASS: src/snapshot.js exists"
fi

for f in static/app.js static/index.html static/style.css static/fixture.html; do
    if [[ ! -f "$f" ]]; then
        echo "FAIL: $f not found"
        FAILED=1
    else
        echo "PASS: $f exists"
    fi
done

# Check 3: Python files purged
if [[ -d "jev_ultrafast" ]]; then
    echo "FAIL: jev_ultrafast directory still exists"
    FAILED=1
else
    echo "PASS: jev_ultrafast directory removed"
fi

if [[ -f "pyproject.toml" ]]; then
    echo "FAIL: pyproject.toml still exists"
    FAILED=1
else
    echo "PASS: pyproject.toml removed"
fi

if [[ -f "uv.lock" ]]; then
    echo "FAIL: uv.lock still exists"
    FAILED=1
else
    echo "PASS: uv.lock removed"
fi

if [[ -f "tests/test_agent.py" ]]; then
    echo "FAIL: tests/test_agent.py still exists"
    FAILED=1
else
    echo "PASS: tests/test_agent.py removed"
fi

# Check 4: .env.example contains Obscura vars
if ! grep -q "OBSCURA_CDP_URL" .env.example 2>/dev/null; then
    echo "FAIL: .env.example missing OBSCURA_CDP_URL"
    FAILED=1
else
    echo "PASS: .env.example contains OBSCURA_CDP_URL"
fi

# Check 5: JS validation
if [[ -f "static/app.js" ]]; then
    if ! node --check static/app.js; then
        echo "FAIL: static/app.js failed node syntax check"
        FAILED=1
    else
        echo "PASS: static/app.js syntax valid"
    fi
fi

if [[ -f "src/snapshot.js" ]]; then
    if ! node --check src/snapshot.js; then
        echo "FAIL: src/snapshot.js failed node syntax check"
        FAILED=1
    else
        echo "PASS: src/snapshot.js syntax valid"
    fi
fi

# Check 6: Cargo formatting
if ! cargo fmt --check; then
    echo "FAIL: cargo fmt check failed"
    FAILED=1
else
    echo "PASS: cargo fmt check passed"
fi

# Check 7: Cargo check & clippy & test
if [[ -f "Cargo.toml" && -f "src/lib.rs" && -f "src/main.rs" && -f "src/bin/jev.rs" ]]; then
    if ! cargo check --all-targets; then
        echo "FAIL: cargo check failed"
        FAILED=1
    else
        echo "PASS: cargo check succeeded"
    fi

    if ! cargo clippy -- -D warnings; then
        echo "FAIL: cargo clippy failed"
        FAILED=1
    else
        echo "PASS: cargo clippy succeeded"
    fi

    if ! cargo test; then
        echo "FAIL: cargo test failed"
        FAILED=1
    else
        echo "PASS: cargo test succeeded"
    fi
fi

if [[ $FAILED -ne 0 ]]; then
    echo "=== Task 1 Verification: FAILED ==="
    exit 1
fi

echo "=== Task 1 Verification: ALL CHECKS PASSED ==="
exit 0
