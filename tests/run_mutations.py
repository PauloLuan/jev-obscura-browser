#!/usr/bin/env python3
import subprocess
import sys
from pathlib import Path

TARGET_FILE = Path("src/cdp.rs")

MUTATIONS = [
    {
        "name": "Mutant 1: Corrupt request ID in format_cdp_request",
        "find": '"id": id,',
        "replace": '"id": id + 100,',
        "expected_fail_test": "test_cdp_request_formatting",
    },
    {
        "name": "Mutant 2: Remove ID existence check in parse_cdp_response",
        "find": 'if val.get("id").is_none() {',
        "replace": "if false {",
        "expected_fail_test": "test_cdp_invalid_responses",
    },
    {
        "name": "Mutant 3: Return Ok instead of Err on protocol error in parse_cdp_response",
        "find": "return Err(CdpError::ProtocolError { code, message });",
        "replace": "return Ok(Value::Null);",
        "expected_fail_test": "test_cdp_error_parsing",
    },
    {
        "name": "Mutant 4: Ignore OBSCURA_BIN in find_obscura_binary",
        "find": "if p.is_file() {\n            return Some(p);\n        }",
        "replace": "/* ignore OBSCURA_BIN */",
        "expected_fail_test": "test_supervisor_auto_spawn",
    },
]

def main():
    original_code = TARGET_FILE.read_text(encoding="utf-8")
    killed = 0
    total = len(MUTATIONS)

    print(f"=== Running Mutation Testing ({total} mutants) on {TARGET_FILE} ===")

    try:
        for idx, m in enumerate(MUTATIONS, 1):
            name = m["name"]
            find_str = m["find"]
            replace_str = m["replace"]

            if find_str not in original_code:
                print(f"ERROR: Could not find target string for {name}")
                sys.exit(1)

            # Apply mutant
            mutated_code = original_code.replace(find_str, replace_str, 1)
            TARGET_FILE.write_text(mutated_code, encoding="utf-8")

            # Run test suite
            res = subprocess.run(
                ["cargo", "test", "--test", "test_cdp"],
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
            )

            # Restore immediately
            TARGET_FILE.write_text(original_code, encoding="utf-8")

            if res.returncode != 0:
                print(f"[{idx}/{total}] KILLED: {name}")
                killed += 1
            else:
                print(f"[{idx}/{total}] SURVIVED: {name} (unexpected pass!)")

    finally:
        # Guarantee original is restored
        TARGET_FILE.write_text(original_code, encoding="utf-8")

    print(f"=== Mutation Result: {killed}/{total} killed ===")
    if killed != total:
        sys.exit(1)

if __name__ == "__main__":
    main()
