use std::sync::Arc;

use sha2::{Digest, Sha256};

use crate::cdp::{CdpClient, CdpError};
use crate::types::{Action, PageState};

pub const SNAPSHOT_JS: &str = include_str!("snapshot.js");

#[derive(Debug, thiserror::Error)]
pub enum BrowserError {
    #[error("Stale page: {0}")]
    StalePage(String),
    #[error("Target not found: {0}")]
    TargetNotFound(String),
    #[error("CDP error: {0}")]
    Cdp(#[from] CdpError),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Execution error: {0}")]
    ExecutionError(String),
}

/// Computes a deterministic SHA-256 fingerprint for a PageState from (url, text, actions, scroll).
pub fn fingerprint(page: &PageState) -> String {
    #[derive(serde::Serialize)]
    struct FingerprintPayload<'a> {
        actions: &'a [Action],
        scroll: (i64, i64),
        text: &'a str,
        url: &'a str,
    }

    let payload = FingerprintPayload {
        actions: &page.actions,
        scroll: page.scroll,
        text: &page.text,
        url: &page.url,
    };

    let serialized = serde_json::to_string(&payload).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(serialized.as_bytes());
    let result = hasher.finalize();
    format!("{:x}", result)
}

/// High-level browser perception and DOM interaction controller.
pub struct Browser {
    pub client: Arc<CdpClient>,
    pub target_id: Option<String>,
    pub session_id: Option<String>,
    pub after_input: Option<Action>,
}

impl Browser {
    pub fn new(client: Arc<CdpClient>, session_id: Option<String>) -> Self {
        Self {
            client,
            target_id: None,
            session_id,
            after_input: None,
        }
    }

    /// Creates and attaches to a browser target, sets viewport metrics, and navigates to `url`.
    pub async fn open(&mut self, url: &str) -> Result<(), BrowserError> {
        if self.target_id.is_none() && self.session_id.is_none() {
            let target_res = self
                .client
                .call(
                    "Target.createTarget",
                    serde_json::json!({
                        "url": "about:blank",
                        "background": true,
                    }),
                    None,
                )
                .await?;

            let target_id = target_res
                .get("targetId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    BrowserError::ExecutionError("Target.createTarget missing targetId".into())
                })?
                .to_string();

            let attach_res = self
                .client
                .call(
                    "Target.attachToTarget",
                    serde_json::json!({
                        "targetId": &target_id,
                        "flatten": true,
                    }),
                    None,
                )
                .await?;

            let session_id = attach_res
                .get("sessionId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    BrowserError::ExecutionError("Target.attachToTarget missing sessionId".into())
                })?
                .to_string();

            self.target_id = Some(target_id);
            self.session_id = Some(session_id);

            let sid = self.session_id.as_deref();
            self.client
                .call(
                    "Emulation.setDeviceMetricsOverride",
                    serde_json::json!({
                        "width": 1120,
                        "height": 780,
                        "deviceScaleFactor": 1,
                        "mobile": false,
                    }),
                    sid,
                )
                .await?;

            self.client
                .call(
                    "Emulation.setFocusEmulationEnabled",
                    serde_json::json!({
                        "enabled": true,
                    }),
                    sid,
                )
                .await?;
        }

        let sid = self.session_id.as_deref();
        self.client
            .call("Page.navigate", serde_json::json!({ "url": url }), sid)
            .await?;

        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(15);
        while start.elapsed() < timeout {
            let eval_res = self
                .client
                .call(
                    "Runtime.evaluate",
                    serde_json::json!({
                        "expression": "document.readyState",
                        "returnByValue": true,
                    }),
                    sid,
                )
                .await;

            if let Ok(val) = eval_res {
                if val
                    .get("result")
                    .and_then(|r| r.get("value"))
                    .and_then(|v| v.as_str())
                    == Some("complete")
                {
                    break;
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }

        Ok(())
    }

    /// Observes the current DOM using `snapshot.js` and optionally captures a screenshot.
    pub async fn observe(&mut self, screenshot: bool) -> Result<PageState, BrowserError> {
        let sid = self.session_id.as_deref();

        // Stabilization helper if previous action needs settlement (e.g. combobox autocomplete)
        if let Some(action) = self.after_input.take() {
            let action_json = serde_json::to_string(&action).unwrap_or_default();
            let expr = format!(
                r#"(action => new Promise(resolve => {{
                  const field = window.__jevFast?.nodes.get(action.node);
                  const autocomplete = action.kind === 'fill' && field?.getAttribute('role') === 'combobox';
                  let frames = 0, stopped = false;
                  const finish = () => {{ stopped = true; resolve(); }};
                  setTimeout(finish, autocomplete ? 200 : 50);
                  const ready = () => {{
                    if (stopped) return;
                    const ids = (field?.getAttribute('aria-controls') || field?.getAttribute('aria-owns') || '')
                      .split(/\s+/).filter(Boolean);
                    const roots = ids.length ? ids.map(id => document.getElementById(id)).filter(Boolean) : [document];
                    const options = roots.flatMap(root => [...root.querySelectorAll('[role="option"]')]);
                    if (++frames >= 2 && (!autocomplete || options.some(e => {{
                      const r = e.getBoundingClientRect();
                      return r.width && r.height && r.bottom > 0 && r.top < innerHeight &&
                        e.checkVisibility({{ checkOpacity: true, checkVisibilityCSS: true }});
                    }}))) finish();
                    else requestAnimationFrame(ready);
                  }};
                  requestAnimationFrame(ready);
                }}))({})"#,
                action_json
            );
            let _ = self
                .client
                .call(
                    "Runtime.evaluate",
                    serde_json::json!({
                        "expression": expr,
                        "awaitPromise": true,
                        "returnByValue": true,
                    }),
                    sid,
                )
                .await;
        }

        for attempt in 0..10 {
            let eval_res = self
                .client
                .call(
                    "Runtime.evaluate",
                    serde_json::json!({
                        "expression": SNAPSHOT_JS,
                        "returnByValue": true,
                    }),
                    sid,
                )
                .await;

            match eval_res {
                Ok(val) => {
                    if val.get("exceptionDetails").is_some() {
                        if attempt == 9 {
                            return Err(BrowserError::StalePage(
                                "Document changed during evaluation".into(),
                            ));
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                        continue;
                    }

                    let value_opt = val
                        .get("result")
                        .and_then(|r| r.get("value"))
                        .filter(|v| !v.is_null());

                    if let Some(info_val) = value_opt {
                        let mut page: PageState = serde_json::from_value(info_val.clone())?;
                        page.fingerprint = fingerprint(&page);

                        if screenshot {
                            let shot_res = self
                                .client
                                .call(
                                    "Page.captureScreenshot",
                                    serde_json::json!({
                                        "format": "jpeg",
                                        "quality": 72,
                                    }),
                                    sid,
                                )
                                .await;

                            if let Ok(shot_val) = shot_res {
                                if let Some(data) = shot_val.get("data").and_then(|d| d.as_str()) {
                                    page.screenshot = Some(data.to_string());
                                }
                            }
                        }

                        return Ok(page);
                    } else {
                        if attempt == 9 {
                            return Err(BrowserError::StalePage("Document is navigating".into()));
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                        continue;
                    }
                }
                Err(_) => {
                    if attempt == 9 {
                        return Err(BrowserError::StalePage("Page did not settle".into()));
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                }
            }
        }

        Err(BrowserError::StalePage("Page did not settle".into()))
    }

    /// Evaluates element guard or DOM marker to confirm target freshness.
    pub async fn fresh(
        &self,
        page: &PageState,
        action: Option<&Action>,
    ) -> Result<bool, BrowserError> {
        let sid = self.session_id.as_deref();

        if let Some(act) = action {
            if act.kind == "click" || act.kind == "select" || act.kind == "fill" {
                if let Some(node) = act.node {
                    let expr = format!(
                        "(() => {{ const c=window.__jevFast; return c ? [c.pageKey(),c.guard(c.nodes.get({}))] : null; }})()",
                        node
                    );
                    let res = self
                        .client
                        .call(
                            "Runtime.evaluate",
                            serde_json::json!({
                                "expression": expr,
                                "returnByValue": true,
                            }),
                            sid,
                        )
                        .await?;

                    if let Some(val) = res.get("result").and_then(|r| r.get("value")) {
                        let expected = serde_json::json!([
                            page.page_key.clone().unwrap_or(serde_json::Value::Null),
                            page.guards
                                .as_ref()
                                .and_then(|g| g.get(node.to_string()))
                                .cloned()
                                .unwrap_or(serde_json::Value::Null)
                        ]);
                        return Ok(val == &expected);
                    }
                    return Ok(false);
                }
            }
        }

        if let Some(ref expected_marker) = page.marker {
            let marker_expr = format!(
                "(() => {{ const state = {}; return state?.marker ?? null; }})()",
                SNAPSHOT_JS
            );
            let res = self
                .client
                .call(
                    "Runtime.evaluate",
                    serde_json::json!({
                        "expression": marker_expr,
                        "returnByValue": true,
                    }),
                    sid,
                )
                .await?;

            if let Some(val) = res.get("result").and_then(|r| r.get("value")) {
                return Ok(val == expected_marker);
            }
            return Ok(false);
        }

        Ok(true)
    }

    /// Executes an action in the browser.
    pub async fn act(
        &mut self,
        action: &Action,
        page: &PageState,
        text: Option<&str>,
    ) -> Result<serde_json::Value, BrowserError> {
        let sid = self.session_id.as_deref();

        if !self.fresh(page, Some(action)).await? {
            return Err(BrowserError::StalePage(
                "Page changed since this decision. Observe again.".into(),
            ));
        }

        let kind = action.kind.as_str();
        if kind == "wait" {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            self.after_input = None;
            return Ok(serde_json::json!({ "executed": action.id }));
        }

        if kind == "scroll" {
            let delta = action.delta.unwrap_or(560);
            self.client
                .call(
                    "Input.dispatchMouseEvent",
                    serde_json::json!({
                        "type": "mouseWheel",
                        "x": 550,
                        "y": 650,
                        "deltaX": 0,
                        "deltaY": delta,
                    }),
                    sid,
                )
                .await?;
            self.after_input = Some(action.clone());
            return Ok(serde_json::json!({ "executed": action.id }));
        }

        // Interactive actions: click, fill, select
        let _node = action
            .node
            .ok_or_else(|| BrowserError::ExecutionError("Invalid observed node".into()))?;

        let action_json = serde_json::to_string(&action)?;
        let hit_test_expr = format!(
            r#"(action => {{
              const e = window.__jevFast?.nodes.get(action.node);
              if (!e?.isConnected || e.matches(':disabled') || e.closest('[aria-disabled="true"],[inert]') ||
                  !e.checkVisibility({{ checkOpacity: true, checkVisibilityCSS: true }})) return null;
              if (action.kind === 'fill' && (e.readOnly || e.getAttribute('aria-readonly') === 'true')) return null;
              const r = e.getBoundingClientRect(), x = r.x + r.width / 2, y = r.y + r.height / 2;
              if (!r.width || !r.height || x < 0 || y < 0 || x >= innerWidth || y >= innerHeight) return null;
              if (!e.contains(document.elementFromPoint(x, y))) return null;
              if (action.kind === 'select') {{
                if (e.tagName !== 'SELECT' || ![...e.options].some(o => o.value === action.value &&
                    !o.disabled && !o.closest('optgroup[disabled]'))) return null;
                e.value = action.value;
                e.dispatchEvent(new Event('input', {{ bubbles: true }}));
                e.dispatchEvent(new Event('change', {{ bubbles: true }}));
              }}
              return {{ x, y }};
            }})({})"#,
            action_json
        );

        let eval_res = self
            .client
            .call(
                "Runtime.evaluate",
                serde_json::json!({
                    "expression": hit_test_expr,
                    "returnByValue": true,
                }),
                sid,
            )
            .await?;

        if eval_res.get("exceptionDetails").is_some() {
            if kind == "select" {
                return Err(BrowserError::ExecutionError(
                    "Dropdown execution was interrupted; inspect before retrying.".into(),
                ));
            }
            return Err(BrowserError::StalePage(
                "Document changed during evaluation".into(),
            ));
        }

        let target_val = eval_res
            .get("result")
            .and_then(|r| r.get("value"))
            .filter(|v| !v.is_null());

        let target = match target_val {
            Some(t) => t,
            None => {
                if kind == "select" {
                    return Err(BrowserError::ExecutionError(
                        "Dropdown execution was not confirmed; inspect before retrying.".into(),
                    ));
                }
                return Err(BrowserError::StalePage(
                    "Target changed or is covered. Observe again.".into(),
                ));
            }
        };

        if kind != "select" {
            let x = target.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let y = target.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);

            for event in ["mousePressed", "mouseReleased"] {
                self.client
                    .call(
                        "Input.dispatchMouseEvent",
                        serde_json::json!({
                            "type": event,
                            "x": x,
                            "y": y,
                            "button": "left",
                            "clickCount": 1,
                        }),
                        sid,
                    )
                    .await?;
            }

            if kind == "fill" {
                let modifiers = if cfg!(target_os = "macos") { 4 } else { 2 };
                self.client
                    .call(
                        "Input.dispatchKeyEvent",
                        serde_json::json!({
                            "type": "keyDown",
                            "key": "a",
                            "code": "KeyA",
                            "modifiers": modifiers,
                            "commands": ["selectAll"],
                        }),
                        sid,
                    )
                    .await?;

                self.client
                    .call(
                        "Input.dispatchKeyEvent",
                        serde_json::json!({
                            "type": "keyUp",
                            "key": "a",
                            "code": "KeyA",
                            "modifiers": modifiers,
                        }),
                        sid,
                    )
                    .await?;

                let text_to_insert = text.unwrap_or("");
                self.client
                    .call(
                        "Input.insertText",
                        serde_json::json!({
                            "text": text_to_insert,
                        }),
                        sid,
                    )
                    .await?;
            }
        }

        self.after_input = Some(action.clone());
        Ok(serde_json::json!({ "executed": action.id }))
    }

    /// Closes the target tab if created by this instance.
    pub async fn close(&mut self) -> Result<(), BrowserError> {
        if let Some(target_id) = self.target_id.take() {
            let _ = self
                .client
                .call(
                    "Target.closeTarget",
                    serde_json::json!({
                        "targetId": target_id,
                    }),
                    None,
                )
                .await;
        }
        self.session_id = None;
        Ok(())
    }
}
