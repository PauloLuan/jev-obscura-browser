# Task 3: Asynchronous WebSocket CDP Client & Process Supervisor in Rust Specification

**File Path:** `/mnt/paulo_home/home/sources/pauloluan/jev-obscura-browser/docs/superpowers/specs/2026-09-18-task-3-cdp-spec.md`  
**Task ID:** Task 3 (#1)  
**Parent Spec:** `/mnt/paulo_home/home/sources/pauloluan/jev-obscura-browser/docs/superpowers/specs/2026-09-18-jev-obscura-migration-design.md`  
**Parent Plan:** `/mnt/paulo_home/home/sources/pauloluan/jev-obscura-browser/docs/superpowers/plans/2026-09-18-jev-obscura-migration.md`  
**Workspace:** `/mnt/paulo_home/home/sources/pauloluan/jev-obscura-browser/.worktrees/task-3`  
**Status:** Approved (100% YOLO Mode)

---

## 1. Objective

Implement an asynchronous Chrome DevTools Protocol (CDP) client over WebSocket and a resilient process supervisor in Rust (`src/cdp.rs`), exported via `src/lib.rs`. Ensure full offline testing (`tests/test_cdp.rs`) covering frame serialization/deserialization, concurrent message dispatching with integer IDs, event filtering, error mapping, connection teardown, and process discovery/supervision.

---

## 2. Acceptance Criteria & Gherkin Scenarios

### Scenario 1: CDP Request Frame Formatting
- **Given** an integer request ID `42`, method `"Target.createTarget"`, JSON parameters `{"url": "about:blank"}`, and `session_id: None`.
- **When** `format_cdp_request(id, method, params, session_id)` is called.
- **Then** the resulting JSON object contains:
  1. `"id": 42` (integer)
  2. `"method": "Target.createTarget"` (string)
  3. `"params": {"url": "about:blank"}` (object)
  4. The `"sessionId"` key is completely absent.
- **And Given** `session_id: Some("sess-123")`:
- **Then** the resulting JSON object contains `"sessionId": "sess-123"`.

### Scenario 2: CDP Response Parsing & Error Mapping
- **Given** a raw JSON-RPC response `{"id": 1, "result": {"targetId": "abc"}}`.
- **When** `parse_cdp_response(val)` is called.
- **Then** it returns `Ok(Value::Object({"targetId": "abc"}))`.
- **And Given** an error JSON-RPC response `{"id": 2, "error": {"code": -32000, "message": "Evaluation failed"}}`.
- **Then** it returns `Err(CdpError::ProtocolError { code: -32000, message: "Evaluation failed".into() })`.
- **And Given** a malformed response without `"id"` or without either `"result"` or `"error"`.
- **Then** it returns `Err(CdpError::ProtocolError { .. })` indicating invalid JSON-RPC format.

### Scenario 3: Asynchronous WebSocket CDP Client Round-Trip
- **Given** a mock WebSocket server listening on a local TCP address.
- **When** `CdpClient::connect(ws_url).await` establishes a WebSocket connection.
- **And** `client.send_request("Runtime.evaluate", json!({"expression": "1+1"}), None).await` is invoked.
- **Then** the server receives the formatted CDP request with an auto-incremented `id`.
- **When** the server replies with `{"id": <id>, "result": {"result": {"value": 2}}}`.
- **Then** `send_request` receives and returns `Ok(json!({"result": {"value": 2}}))`.
- **And** calling `client.call(...)` produces the identical result as an alias for `send_request`.

### Scenario 4: Event Filtering & Concurrent Request Multiplexing
- **Given** multiple concurrent calls to `send_request(...)` from separate Tokio tasks.
- **And** the mock server sends unprompted CDP events (e.g. `{"method": "Target.targetCreated", "params": {}}`) interspersed with responses out of order.
- **Then** each caller receives its matching response mapped by `id`.
- **And** CDP events without matching pending `id`s are dropped without crashing or deadlocking the client.

### Scenario 5: Connection Drop & Request Timeout Handling
- **Given** a pending request awaiting response.
- **When** the WebSocket connection drops or closes.
- **Then** the pending request immediately completes with `Err(CdpError::ConnectionClosed)`.
- **And When** a request exceeds its configured timeout via `send_request_timeout`.
- **Then** it returns `Err(CdpError::Timeout)` and the pending map entry is cleaned up.

### Scenario 6: Process Supervisor — Already Running Instance
- **Given** an open TCP port on `127.0.0.1:<port>`.
- **When** `ensure_obscura(Some("127.0.0.1:<port>")).await` is called.
- **Then** it immediately succeeds without inspecting `PATH` or spawning child processes, returning a valid `ws://...` endpoint.

### Scenario 7: Process Supervisor — Binary Missing
- **Given** a closed TCP port and no `obscura` executable in `PATH` or `OBSCURA_BIN`.
- **When** `ensure_obscura(Some("127.0.0.1:<unreachable_port>")).await` is called.
- **Then** it returns `Err(CdpError::SupervisorError(msg))` where `msg` contains:
  `"Obscura browser binary not found. Please install obscura (https://obscura.sh/) or start via Docker: docker run -p 9222:9222 ghcr.io/obscura/obscura"`.

### Scenario 8: Process Supervisor — Auto-Spawn When Binary Exists
- **Given** a closed TCP port and a mock executable binary specified in `OBSCURA_BIN`.
- **When** `ensure_obscura(Some("127.0.0.1:<port>")).await` is called.
- **Then** it executes the binary with arguments `serve --port 9222 --allow-file-access --allow-private-network`, polls readiness until port opens, and returns the WebSocket URL.

---

## 3. Negative Constraints

1. **Zero External Network Calls:** 100% of tests must run against local loopback listeners (`127.0.0.1:0`).
2. **Zero Async Locks Across `.await`:** Mutex locks protecting pending message tables must only be held synchronously during hash map insert/remove operations, never across await points.
3. **No Retries on Browser Mutations:** Requests are sent once; caller decides higher-level retry policies.
4. **Zero Warnings:** All code must pass `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check`.

---

## 4. Architecture & Module Design

### `src/cdp.rs`

```rust
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio_tungstenite::tungstenite::Message;

#[derive(Debug, thiserror::Error)]
pub enum CdpError {
    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Protocol error ({code}): {message}")]
    ProtocolError { code: i64, message: String },
    #[error("Timeout awaiting response")]
    Timeout,
    #[error("Connection closed")]
    ConnectionClosed,
    #[error("Supervisor error: {0}")]
    SupervisorError(String),
}

pub fn format_cdp_request(
    id: u64,
    method: &str,
    params: Value,
    session_id: Option<&str>,
) -> Value {
    let mut req = serde_json::json!({
        "id": id,
        "method": method,
        "params": params,
    });
    if let Some(sid) = session_id {
        req["sessionId"] = Value::String(sid.to_string());
    }
    req
}

pub fn parse_cdp_response(val: Value) -> Result<Value, CdpError> {
    if !val.is_object() {
        return Err(CdpError::ProtocolError {
            code: -32600,
            message: "Invalid JSON-RPC format: not an object".into(),
        });
    }

    if let Some(err_obj) = val.get("error") {
        let code = err_obj.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
        let message = err_obj
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("Unknown protocol error")
            .to_string();
        return Err(CdpError::ProtocolError { code, message });
    }

    if val.get("id").is_none() {
        return Err(CdpError::ProtocolError {
            code: -32600,
            message: "Invalid JSON-RPC response: missing id".into(),
        });
    }

    if let Some(result) = val.get("result") {
        Ok(result.clone())
    } else {
        Err(CdpError::ProtocolError {
            code: -32600,
            message: "Invalid JSON-RPC response: missing result or error".into(),
        })
    }
}

pub struct CdpClient {
    next_id: AtomicU64,
    out_tx: mpsc::UnboundedSender<Message>,
    pending: Arc<std::sync::Mutex<HashMap<u64, oneshot::Sender<Result<Value, CdpError>>>>>,
}

impl CdpClient {
    pub async fn connect(ws_url: &str) -> Result<Self, CdpError>;
    pub async fn send_request(
        &self,
        method: &str,
        params: Value,
        session_id: Option<&str>,
    ) -> Result<Value, CdpError>;
    pub async fn send_request_timeout(
        &self,
        method: &str,
        params: Value,
        session_id: Option<&str>,
        timeout: Duration,
    ) -> Result<Value, CdpError>;
    pub async fn call(
        &self,
        method: &str,
        params: Value,
        session_id: Option<&str>,
    ) -> Result<Value, CdpError>;
}

pub async fn ensure_obscura(cdp_url: Option<&str>) -> Result<String, CdpError>;
```

---

## 5. Verification Plan

1. **RED Phase:**
   - Create stub `src/cdp.rs` where functions panic with `todo!()` or return stubs.
   - Export `pub mod cdp;` in `src/lib.rs`.
   - Write comprehensive test cases in `tests/test_cdp.rs`.
   - Execute `cargo test --test test_cdp` and observe RED failures.
2. **GREEN Phase:**
   - Implement complete logic in `src/cdp.rs`.
   - Run `cargo test --test test_cdp` and verify all tests pass.
3. **GAUNTLET Phase:**
   - `cargo check --all-targets`
   - `cargo clippy --all-targets -- -D warnings`
   - `cargo test` (all unit & integration suites)
   - `cargo fmt --check`
   - `node --check static/app.js && node --check src/snapshot.js`
   - Negative controls & manual mutation kills.
4. **EVIDENCE Phase:**
   - Write `/mnt/paulo_home/home/sources/pauloluan/jev-obscura-browser/.luan-coder/20260918-171300/task-3-evidence.md`.
5. **JJ Revision Checkpoint:**
   - Describe commit with `jj describe -m "feat(cdp): implement async WebSocket CDP client and process supervisor in Rust (Closes #1)"`.
