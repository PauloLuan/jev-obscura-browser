#!/usr/bin/env python3
import subprocess
import sys
from pathlib import Path

MUTATIONS = [
    {
        "name": "Mutant 1: Corrupt default serve port in CLI",
        "file": Path("src/demo.rs"),
        "find": "#[arg(short, long, default_value_t = 8766)]",
        "replace": "#[arg(short, long, default_value_t = 9999)]",
        "test": "test_cli_arg_parsing",
    },
    {
        "name": "Mutant 2: Corrupt Content-Type in index_handler",
        "file": Path("src/demo.rs"),
        "find": '[(header::CONTENT_TYPE, "text/html; charset=utf-8")]',
        "replace": '[(header::CONTENT_TYPE, "application/json")]',
        "test": "test_index_route",
    },
    {
        "name": "Mutant 3: Corrupt ready status in start_handler",
        "file": Path("src/demo.rs"),
        "find": 'app.status = "ready".to_string();',
        "replace": 'app.status = "idle".to_string();',
        "test": "test_api_start_and_reset",
    },
    {
        "name": "Mutant 4: Corrupt initial status in AppState::default",
        "file": Path("src/demo.rs"),
        "find": 'status: "idle".to_string(),',
        "replace": 'status: "ready".to_string(),',
        "test": "test_api_state_empty",
    },
    {
        "name": "Mutant 5: Omit history recording in step_handler",
        "file": Path("src/demo.rs"),
        "find": "app.history.push(decision);",
        "replace": "// app.history.push(decision);",
        "test": "test_api_step",
    },
]

def main():
    killed = 0
    total = len(MUTATIONS)

    print(f"=== Running Mutation Testing ({total} mutants) on Task 5 (src/demo.rs) ===")

    for idx, m in enumerate(MUTATIONS, 1):
        target_file = m["file"]
        original_code = target_file.read_text(encoding="utf-8")
        name = m["name"]
        find_str = m["find"]
        replace_str = m["replace"]
        test_name = m["test"]

        if find_str not in original_code:
            print(f"ERROR: Could not find target string in {target_file} for {name}")
            sys.exit(1)

        try:
            mutated_code = original_code.replace(find_str, replace_str, 1)
            target_file.write_text(mutated_code, encoding="utf-8")

            res = subprocess.run(
                ["cargo", "test", "--test", "test_server", "--", test_name],
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
            )

            if res.returncode != 0:
                print(f"[{idx}/{total}] KILLED: {name}")
                killed += 1
            else:
                print(f"[{idx}/{total}] SURVIVED: {name} (unexpected pass!)")
        finally:
            target_file.write_text(original_code, encoding="utf-8")

    print(f"=== Mutation Result: {killed}/{total} killed ===")
    if killed != total:
        sys.exit(1)

if __name__ == "__main__":
    main()
