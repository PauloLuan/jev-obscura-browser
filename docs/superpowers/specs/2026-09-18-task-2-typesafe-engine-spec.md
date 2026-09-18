# Task 2: Data Types & TypeSafe Decision Engine in Rust Specification

**File Path:** `/mnt/paulo_home/home/sources/pauloluan/jev-obscura-browser/docs/superpowers/specs/2026-09-18-task-2-typesafe-engine-spec.md`  
**Task ID:** Task 2 (#1)  
**Parent Spec:** `/mnt/paulo_home/home/sources/pauloluan/jev-obscura-browser/docs/superpowers/specs/2026-09-18-jev-obscura-migration-design.md`  
**Parent Plan:** `/mnt/paulo_home/home/sources/pauloluan/jev-obscura-browser/docs/superpowers/plans/2026-09-18-jev-obscura-migration.md`  
**Workspace:** `/mnt/paulo_home/home/sources/pauloluan/jev-obscura-browser/.worktrees/task-2`  
**Status:** Approved (100% YOLO Mode)

---

## 1. Objective

Implement core domain types (`src/types.rs`), TypeSafe dynamic prompt & criteria generation (`src/questions.rs`), and the TypeSafe decision engine and text helper (`src/model.rs`) in Rust, exported via `src/lib.rs`. Ensure full offline contract testing with `wiremock` in `tests/test_model.rs` validating 1-round-trip speculative decisions, rigorous choice probability checks, and action space partitioning.

---

## 2. Acceptance Criteria & Gherkin Scenarios

### Scenario 1: Action Space Partitioning
- **Given** an observed sequence of `Action` items containing interactive actions (e.g. `fill`, `click`, `select`) and control actions (e.g. `wait`, `scroll_down`).
- **When** `action_space(&actions)` is invoked.
- **Then** it returns a tuple `(elements, targets, controls)`:
  1. `elements: HashMap<String, Action>` maps action IDs to their full `Action` definition.
  2. `targets: HashMap<String, Vec<String>>` groups allowed action IDs by uppercase operation (`CLICK`, `TYPE_TEXT`, `SELECT`).
  3. `controls: Vec<String>` contains uppercase control operation names (`WAIT`, `DONE`, `SCROLL_UP`, `SCROLL_DOWN`).

### Scenario 2: Choice Validation
- **Given** a TypeSafe `Choice` response and a set of allowed candidate IDs.
- **When** `validate_choice(&choice, &allowed)` is invoked.
- **Then** it returns `Ok(())` if and only if:
  1. `choice.choice` is a member of `allowed`.
  2. `choice.probabilities` keys exactly match `allowed`.
  3. All probabilities and confidence are finite numbers in the range `[0.0, 1.0]`.
  4. The sum of probabilities is within `0.02` of `1.0`.
  5. The selected choice has probability `>= max(probabilities) - 1e-6`.
- **And** returns `Err(ModelError::InvalidChoice)` if any condition is violated.

### Scenario 3: 1-Round-Trip TypeSafe Decision with Speculative Heads
- **Given** a `PageState`, user goal, and execution history.
- **When** `choose(...)` is called against a TypeSafe endpoint (mocked via `wiremock`).
- **Then** exactly ONE HTTP POST request is sent to `/v1/systemone` containing:
  1. The `operation` question with criteria for all available operations (`CLICK`, `TYPE_TEXT`, `SELECT`, `WAIT`, `DONE`, etc.).
  2. Speculative target head questions (`click_target`, `type_text_target`, `select_target`) for each active operation.
- **And** when TypeSafe responds with answers:
  1. The chosen `operation` is validated.
  2. If the operation is an interactive operation (`CLICK`, `TYPE_TEXT`, `SELECT`), ONLY the corresponding speculative target head is validated and consumed; unselected target heads are ignored even if malformed.
  3. Returns a `Decision { operation, target, text: None }`.

### Scenario 4: Text Helper for Editable Fields
- **Given** field context containing goal, field metadata, and page context.
- **When** `field_text(...)` is called against an OpenAI-compatible endpoint (mocked via `wiremock`).
- **Then** a POST request is sent to `/chat/completions` requesting a JSON response with schema `{"text": "..."}`.
- **And** returns `Ok(String)` when a valid non-empty string is provided.
- **And** returns `Err(ModelError)` if the credential `TEXT_MODEL_API_KEY` is missing or the response cannot be parsed.

---

## 3. Negative Constraints

1. **Zero External Network Calls in Tests:** All tests in `tests/test_model.rs` must execute offline using `wiremock` or pure unit evaluation.
2. **No Arbitrary Selectors or Executable Code:** Targets must map strictly to observed element/action IDs.
3. **Unselected Target Head Isolation:** Malformed or invalid answers in unused target heads must NOT fail the request if the chosen operation's head is valid.
4. **Zero Warnings:** All code must pass `cargo clippy -- -D warnings` and `cargo fmt --check`.

---

## 4. Architecture & Module Design

### `src/types.rs`
```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Action {
    pub id: String,
    pub kind: String,
    pub label: String,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub node: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rect: Option<Rect>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delta: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checked: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expanded: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Choice {
    pub choice: String,
    pub confidence: f64,
    pub probabilities: HashMap<String, f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Decision {
    pub operation: String,
    pub target: Option<String>,
    pub text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PageState {
    pub url: String,
    #[serde(default)]
    pub title: String,
    pub text: String,
    pub actions: Vec<Action>,
    #[serde(deserialize_with = "deserialize_scroll")]
    pub scroll: (i64, i64),
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screenshot: Option<String>,
}
```

### `src/questions.rs`
Defines instructions (`NEXT_ACTION`, `TARGET`, `TEXT_VALUE`), allowed operation list (`ALLOWED_OPERATIONS`), and helper builders:
- `build_questions(goal: &str, targets: &HashMap<String, Vec<String>>, elements: &HashMap<String, Action>, controls: &[String]) -> Value`

### `src/model.rs`
- `action_space(actions: &[Action]) -> (HashMap<String, Action>, HashMap<String, Vec<String>>, Vec<String>)`
- `validate_choice(choice: &Choice, allowed: &HashSet<String>) -> Result<(), ModelError>`
- `choose(client, base_url, api_key, model, page, goal, history) -> Result<Decision, ModelError>`
- `field_text(client, base_url, api_key, model, context) -> Result<String, ModelError>`

---

## 5. Verification Plan

1. **RED Phase:**
   - Create `tests/test_model.rs` with test cases:
     - `test_invalid_choice_rejected`
     - `test_action_space_partitioning`
     - `test_typesafe_request_building`
     - `test_choose_mock_prediction` (via `wiremock`)
     - `test_field_text_mock_prediction` (via `wiremock`)
   - Run `cargo test --test test_model` -> MUST fail compilation or execution.
2. **GREEN Phase:**
   - Implement `src/types.rs`, `src/questions.rs`, `src/model.rs`, `src/lib.rs`.
   - Run `cargo test --test test_model` -> MUST pass.
3. **GAUNTLET Phase:**
   - `cargo check --all-targets`
   - `cargo clippy -- -D warnings`
   - `cargo test`
   - `cargo fmt --check`
   - Manual mutation kills / negative controls.
