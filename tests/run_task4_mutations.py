#!/usr/bin/env python3
import subprocess
import sys
from pathlib import Path

MUTATIONS = [
    {
        "name": "Mutant 1: Corrupt fingerprint input payload url",
        "file": Path("src/browser.rs"),
        "find": 'url: &page.url,',
        "replace": 'url: "corrupted",',
        "test": "test_fingerprint_differs_on_mutation",
    },
    {
        "name": "Mutant 2: Bypass freshness check in Browser::act",
        "file": Path("src/browser.rs"),
        "find": 'if !self.fresh(page, Some(action)).await? {',
        "replace": 'if false {',
        "test": "test_browser_stale_page_rejected",
    },
    {
        "name": "Mutant 3: Corrupt delta in Browser::act scroll",
        "file": Path("src/browser.rs"),
        "find": 'let delta = action.delta.unwrap_or(560);',
        "replace": 'let delta = 999999;',
        "test": "test_browser_act_scroll_mock",
    },
    {
        "name": "Mutant 4: Bypass field_text invocation in Agent::step",
        "file": Path("src/agent.rs"),
        "find": 'if decision.operation == "TYPE_TEXT" && decision.text.is_none() {',
        "replace": 'if false {',
        "test": "test_agent_step_type_text_with_llm",
    },
    {
        "name": "Mutant 5: Return Ok instead of MaxStepsExceeded in Agent::run",
        "file": Path("src/agent.rs"),
        "find": 'return Err(AgentError::MaxStepsExceeded);',
        "replace": 'return Ok(history);',
        "test": "test_agent_run_max_steps_budget",
    },
]

def main():
    killed = 0
    total = len(MUTATIONS)

    print(f"=== Running Mutation Testing ({total} mutants) on Task 4 (src/browser.rs & src/agent.rs) ===")

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
                ["cargo", "test", "--test", "test_browser", "--", test_name],
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
