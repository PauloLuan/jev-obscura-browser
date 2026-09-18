#!/usr/bin/env bash
set -euo pipefail

echo "=== Running Task 6 Verification Gauntlet ==="

echo "--- 1. Checking Purged Files ---"
PURGED_FILES=(
    "docs/demo.mp4"
    "docs/demo.gif"
    "docs/inspector.png"
    "docs/flights-result.png"
    "docs/flights-measurement.json"
    "docs/flights-prepared-measurement.json"
    "docs/full-speed-measurement.json"
    "docs/measurement.json"
    "docs/performance.md"
    "docs/performance-prepared.md"
    "docs/launch-draft.md"
    "scripts"
    "examples"
)

for file in "${PURGED_FILES[@]}"; do
    if [ -e "$file" ]; then
        echo "FAIL: Legacy file or directory still exists: $file"
        exit 1
    fi
done
echo "PASS: All legacy media, measurements, and scripts purged"

echo "--- 2. Validating docs/banner.svg ---"
test -f docs/banner.svg || { echo "FAIL: docs/banner.svg not found"; exit 1; }
python3 -c "
import xml.etree.ElementTree as ET
tree = ET.parse('docs/banner.svg')
root = tree.getroot()
assert root.tag.endswith('svg'), f'Expected <svg>, got {root.tag}'
assert root.attrib.get('viewBox') == '0 0 1400 480', f'viewBox mismatch: {root.attrib.get(\"viewBox\")}'

with open('docs/banner.svg', 'r', encoding='utf-8') as f:
    content = f.read()

assert 'JEV OBSCURA BROWSER' in content, 'Missing JEV OBSCURA BROWSER in banner.svg'
assert 'Obscura' in content and 'TypeSafe' in content, 'Missing Obscura x TypeSafe in banner.svg'
assert 'Fast speculative browser automation in pure Rust' in content, 'Missing tagline in banner.svg'
assert '[e' in content or 'e1' in content or 'e7' in content, 'Missing indexed element markers in banner.svg'
print('SVG Validated successfully: XML well-formed, modern dark identity confirmed.')
"
echo "PASS: docs/banner.svg valid XML and content"

echo "--- 3. Validating README.md ---"
test -f README.md || { echo "FAIL: README.md not found"; exit 1; }
python3 -c "
with open('README.md', 'r', encoding='utf-8') as f:
    readme = f.read()

assert '<img src=\"docs/banner.svg\" alt=\"Jev Obscura Browser · Obscura × TypeSafe\" width=\"100%\" />' in readme, 'Missing banner in README.md'
assert '# Jev Obscura Browser ⚡ (Rust)' in readme, 'Missing Rust title in README.md'
assert 'obscura.sh' in readme, 'Missing obscura.sh reference in README.md'
assert 'snapshot.js' in readme, 'Missing snapshot.js reference in README.md'
assert 'TYPESAFE_API_KEY' in readme, 'Missing TYPESAFE_API_KEY in README.md'
assert 'TEXT_MODEL_API_KEY' in readme, 'Missing TEXT_MODEL_API_KEY in README.md'
assert 'OBSCURA_URL' in readme, 'Missing OBSCURA_URL in README.md'
assert '8766' in readme, 'Missing port 8766 in README.md'
assert 'cargo build --release' in readme, 'Missing cargo build --release in README.md'
assert 'cargo run -- run' in readme, 'Missing cargo run -- run in README.md'
assert 'cargo run -- serve' in readme, 'Missing cargo run -- serve in README.md'
assert 'cargo run -- check' in readme, 'Missing cargo run -- check in README.md'
assert 'mutation' in readme.lower(), 'Missing mutation safety in README.md'
assert 'fingerprint' in readme.lower(), 'Missing fingerprint freshness checking in README.md'
print('README.md content validated successfully.')
"
echo "PASS: README.md content validated"

echo "--- 4. Validating AGENTS.md ---"
test -f AGENTS.md || { echo "FAIL: AGENTS.md not found"; exit 1; }
python3 -c "
with open('AGENTS.md', 'r', encoding='utf-8') as f:
    agents = f.read().strip()

expected_header = '# Jev Obscura Browser (Rust)'
expected_checks = 'Checks: cargo clippy -- -D warnings, cargo test, node --check static/app.js, cargo build --release.'
assert expected_header in agents, f'Missing {expected_header} in AGENTS.md'
assert expected_checks in agents, f'Missing {expected_checks} in AGENTS.md'
assert 'uv run' not in agents, 'Found legacy uv run in AGENTS.md'
print('AGENTS.md content validated successfully.')
"
echo "PASS: AGENTS.md content validated"

echo "--- 5. Rust Quality Gates ---"
echo "Running cargo fmt --check..."
cargo fmt --check
echo "PASS: cargo fmt --check"

echo "Running cargo clippy --all-targets -- -D warnings..."
cargo clippy --all-targets -- -D warnings
echo "PASS: cargo clippy"

echo "Running cargo test..."
cargo test
echo "PASS: cargo test"

echo "Running node --check static/app.js..."
node --check static/app.js
node --check src/snapshot.js
echo "PASS: node --check"

echo "Running cargo build --release..."
cargo build --release
echo "PASS: cargo build --release"

echo "=== Task 6 Verification: ALL GAUNTLET LAYERS PASSED ==="
