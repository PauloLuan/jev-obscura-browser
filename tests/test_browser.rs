use std::net::SocketAddr;
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::Message;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use jev_obscura_browser::agent::{Agent, AgentConfig, AgentError};
use jev_obscura_browser::browser::{fingerprint, Browser, BrowserError};
use jev_obscura_browser::cdp::CdpClient;
use jev_obscura_browser::types::{Action, PageState, Rect};

fn create_sample_page() -> PageState {
    PageState {
        url: "https://example.com/form".into(),
        title: "Test Form".into(),
        text: "Submit Form\nEnter Details".into(),
        scroll: (0, 800),
        actions: vec![
            Action {
                id: "e1".into(),
                kind: "fill".into(),
                label: "Username".into(),
                role: "textbox".into(),
                value: "".into(),
                node: Some(101),
                rect: Some(Rect {
                    x: 100.0,
                    y: 100.0,
                    w: 200.0,
                    h: 40.0,
                }),
                ..Default::default()
            },
            Action {
                id: "e2".into(),
                kind: "click".into(),
                label: "Submit".into(),
                role: "button".into(),
                value: "".into(),
                node: Some(102),
                rect: Some(Rect {
                    x: 100.0,
                    y: 200.0,
                    w: 100.0,
                    h: 40.0,
                }),
                ..Default::default()
            },
            Action {
                id: "scroll_down".into(),
                kind: "scroll".into(),
                label: "Scroll down".into(),
                delta: Some(560),
                ..Default::default()
            },
            Action {
                id: "wait".into(),
                kind: "wait".into(),
                label: "Wait for update".into(),
                ..Default::default()
            },
        ],
        fingerprint: "".into(),
        screenshot: None,
        marker: Some(json!(["marker-val"])),
        page_key: Some(json!(["page-key-val"])),
        guards: Some(json!({"101": ["guard-101"], "102": ["guard-102"]})),
    }
}

#[test]
fn test_fingerprint_deterministic() {
    let page1 = create_sample_page();
    let fp1 = fingerprint(&page1);
    let fp2 = fingerprint(&page1);

    assert_eq!(fp1, fp2);
    assert_eq!(fp1.len(), 64);
    assert!(fp1.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn test_fingerprint_differs_on_mutation() {
    let base = create_sample_page();
    let base_fp = fingerprint(&base);

    // 1. Mutate url
    let mut mutated_url = base.clone();
    mutated_url.url = "https://example.com/other".into();
    assert_ne!(base_fp, fingerprint(&mutated_url));

    // 2. Mutate text
    let mut mutated_text = base.clone();
    mutated_text.text = "Different Text".into();
    assert_ne!(base_fp, fingerprint(&mutated_text));

    // 3. Mutate actions
    let mut mutated_actions = base.clone();
    mutated_actions.actions.pop();
    assert_ne!(base_fp, fingerprint(&mutated_actions));

    // 4. Mutate scroll
    let mut mutated_scroll = base.clone();
    mutated_scroll.scroll = (100, 800);
    assert_ne!(base_fp, fingerprint(&mutated_scroll));
}

#[tokio::test]
async fn test_browser_observe_mock() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr: SocketAddr = listener.local_addr().expect("addr");
    let ws_url = format!("ws://{}", addr);

    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept");
        let mut ws = tokio_tungstenite::accept_async(stream).await.expect("ws");

        while let Some(msg) = ws.next().await {
            let msg = msg.expect("msg");
            if let Message::Text(text) = msg {
                let req: serde_json::Value = serde_json::from_str(&text).expect("json");
                let id = req["id"].as_u64().expect("id");
                let method = req["method"].as_str().unwrap_or("");

                if method == "Runtime.evaluate" {
                    let snapshot_result = json!({
                        "url": "https://example.com/mock",
                        "title": "Mock Page",
                        "text": "Hello world",
                        "scroll": {"y": 0, "height": 1000},
                        "actions": [
                            {
                                "id": "e1",
                                "kind": "click",
                                "label": "Click me",
                                "role": "button",
                                "value": "",
                                "node": 10
                            }
                        ],
                        "marker": ["mock-marker"],
                        "page_key": ["mock-key"],
                        "guards": {"10": ["guard-10"]},
                        "omitted_actions": 0
                    });

                    let resp = json!({
                        "id": id,
                        "result": {
                            "result": {
                                "value": snapshot_result
                            }
                        }
                    });
                    ws.send(Message::Text(resp.to_string().into()))
                        .await
                        .expect("send");
                }
            }
        }
    });

    let client = Arc::new(CdpClient::connect(&ws_url).await.expect("connect"));
    let mut browser = Browser::new(client, Some("sess-1".into()));

    let page = browser.observe(false).await.expect("observe");
    assert_eq!(page.url, "https://example.com/mock");
    assert_eq!(page.title, "Mock Page");
    assert_eq!(page.actions.len(), 1);
    assert_eq!(page.actions[0].id, "e1");
    assert_eq!(page.fingerprint.len(), 64);
    assert_eq!(page.screenshot, None);
}

#[tokio::test]
async fn test_browser_observe_with_screenshot_mock() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr: SocketAddr = listener.local_addr().expect("addr");
    let ws_url = format!("ws://{}", addr);

    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept");
        let mut ws = tokio_tungstenite::accept_async(stream).await.expect("ws");

        while let Some(msg) = ws.next().await {
            let msg = msg.expect("msg");
            if let Message::Text(text) = msg {
                let req: serde_json::Value = serde_json::from_str(&text).expect("json");
                let id = req["id"].as_u64().expect("id");
                let method = req["method"].as_str().unwrap_or("");

                if method == "Runtime.evaluate" {
                    let snapshot_result = json!({
                        "url": "https://example.com/mock",
                        "title": "Mock Page",
                        "text": "Hello world",
                        "scroll": [0, 1000],
                        "actions": [],
                        "marker": ["mock-marker"],
                        "page_key": ["mock-key"],
                        "guards": {},
                        "omitted_actions": 0
                    });

                    let resp = json!({
                        "id": id,
                        "result": {
                            "result": {
                                "value": snapshot_result
                            }
                        }
                    });
                    ws.send(Message::Text(resp.to_string().into()))
                        .await
                        .expect("send");
                } else if method == "Page.captureScreenshot" {
                    assert_eq!(req["params"]["format"], "jpeg");
                    assert_eq!(req["params"]["quality"], 72);
                    let resp = json!({
                        "id": id,
                        "result": {
                            "data": "base64jpegdata=="
                        }
                    });
                    ws.send(Message::Text(resp.to_string().into()))
                        .await
                        .expect("send");
                }
            }
        }
    });

    let client = Arc::new(CdpClient::connect(&ws_url).await.expect("connect"));
    let mut browser = Browser::new(client, Some("sess-1".into()));

    let page = browser
        .observe(true)
        .await
        .expect("observe with screenshot");
    assert_eq!(page.screenshot, Some("base64jpegdata==".to_string()));
}

#[tokio::test]
async fn test_browser_act_click_mock() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr: SocketAddr = listener.local_addr().expect("addr");
    let ws_url = format!("ws://{}", addr);

    let dispatched_events = Arc::new(Mutex::new(Vec::<String>::new()));
    let dispatched_events_clone = Arc::clone(&dispatched_events);

    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept");
        let mut ws = tokio_tungstenite::accept_async(stream).await.expect("ws");

        while let Some(msg) = ws.next().await {
            let msg = msg.expect("msg");
            if let Message::Text(text) = msg {
                let req: serde_json::Value = serde_json::from_str(&text).expect("json");
                let id = req["id"].as_u64().expect("id");
                let method = req["method"].as_str().unwrap_or("");

                if method == "Runtime.evaluate" {
                    let expr = req["params"]["expression"].as_str().unwrap_or("");
                    if expr.contains("window.__jevFast") && expr.contains("guard") {
                        // Freshness check
                        let resp = json!({
                            "id": id,
                            "result": {
                                "result": {
                                    "value": [["key-1"], ["guard-102"]]
                                }
                            }
                        });
                        ws.send(Message::Text(resp.to_string().into()))
                            .await
                            .expect("send");
                    } else {
                        // Target hit-test resolution
                        let resp = json!({
                            "id": id,
                            "result": {
                                "result": {
                                    "value": {"x": 150.0, "y": 220.0}
                                }
                            }
                        });
                        ws.send(Message::Text(resp.to_string().into()))
                            .await
                            .expect("send");
                    }
                } else if method == "Input.dispatchMouseEvent" {
                    let event_type = req["params"]["type"].as_str().unwrap_or("").to_string();
                    dispatched_events_clone.lock().await.push(event_type);

                    let resp = json!({
                        "id": id,
                        "result": {}
                    });
                    ws.send(Message::Text(resp.to_string().into()))
                        .await
                        .expect("send");
                }
            }
        }
    });

    let client = Arc::new(CdpClient::connect(&ws_url).await.expect("connect"));
    let mut browser = Browser::new(client, Some("sess-1".into()));

    let mut page = create_sample_page();
    page.page_key = Some(json!(["key-1"]));
    page.guards = Some(json!({"102": ["guard-102"]}));

    let action = page.actions[1].clone(); // Submit button (click)
    let res = browser.act(&action, &page, None).await.expect("act click");
    assert_eq!(res["executed"], "e2");

    let events = dispatched_events.lock().await.clone();
    assert_eq!(events, vec!["mousePressed", "mouseReleased"]);
}

#[tokio::test]
async fn test_browser_act_fill_mock() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr: SocketAddr = listener.local_addr().expect("addr");
    let ws_url = format!("ws://{}", addr);

    let key_events = Arc::new(Mutex::new(Vec::<String>::new()));
    let inserted_text = Arc::new(Mutex::new(String::new()));
    let key_events_clone = Arc::clone(&key_events);
    let inserted_text_clone = Arc::clone(&inserted_text);

    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept");
        let mut ws = tokio_tungstenite::accept_async(stream).await.expect("ws");

        while let Some(msg) = ws.next().await {
            let msg = msg.expect("msg");
            if let Message::Text(text) = msg {
                let req: serde_json::Value = serde_json::from_str(&text).expect("json");
                let id = req["id"].as_u64().expect("id");
                let method = req["method"].as_str().unwrap_or("");

                if method == "Runtime.evaluate" {
                    let expr = req["params"]["expression"].as_str().unwrap_or("");
                    if expr.contains("window.__jevFast") && expr.contains("guard") {
                        let resp = json!({
                            "id": id,
                            "result": {
                                "result": {
                                    "value": [["key-1"], ["guard-101"]]
                                }
                            }
                        });
                        ws.send(Message::Text(resp.to_string().into()))
                            .await
                            .expect("send");
                    } else {
                        let resp = json!({
                            "id": id,
                            "result": {
                                "result": {
                                    "value": {"x": 200.0, "y": 120.0}
                                }
                            }
                        });
                        ws.send(Message::Text(resp.to_string().into()))
                            .await
                            .expect("send");
                    }
                } else if method == "Input.dispatchMouseEvent" {
                    let resp = json!({"id": id, "result": {}});
                    ws.send(Message::Text(resp.to_string().into()))
                        .await
                        .expect("send");
                } else if method == "Input.dispatchKeyEvent" {
                    let t = req["params"]["type"].as_str().unwrap_or("").to_string();
                    key_events_clone.lock().await.push(t);
                    let resp = json!({"id": id, "result": {}});
                    ws.send(Message::Text(resp.to_string().into()))
                        .await
                        .expect("send");
                } else if method == "Input.insertText" {
                    let txt = req["params"]["text"].as_str().unwrap_or("").to_string();
                    *inserted_text_clone.lock().await = txt;
                    let resp = json!({"id": id, "result": {}});
                    ws.send(Message::Text(resp.to_string().into()))
                        .await
                        .expect("send");
                }
            }
        }
    });

    let client = Arc::new(CdpClient::connect(&ws_url).await.expect("connect"));
    let mut browser = Browser::new(client, Some("sess-1".into()));

    let mut page = create_sample_page();
    page.page_key = Some(json!(["key-1"]));
    page.guards = Some(json!({"101": ["guard-101"]}));

    let action = page.actions[0].clone(); // Username field (fill)
    let res = browser
        .act(&action, &page, Some("alice_smith"))
        .await
        .expect("act fill");
    assert_eq!(res["executed"], "e1");

    let keys = key_events.lock().await.clone();
    assert_eq!(keys, vec!["keyDown", "keyUp"]);
    assert_eq!(*inserted_text.lock().await, "alice_smith");
}

#[tokio::test]
async fn test_browser_stale_page_rejected() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr: SocketAddr = listener.local_addr().expect("addr");
    let ws_url = format!("ws://{}", addr);

    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept");
        let mut ws = tokio_tungstenite::accept_async(stream).await.expect("ws");

        while let Some(msg) = ws.next().await {
            let msg = msg.expect("msg");
            if let Message::Text(text) = msg {
                let req: serde_json::Value = serde_json::from_str(&text).expect("json");
                let id = req["id"].as_u64().expect("id");
                let method = req["method"].as_str().unwrap_or("");

                if method == "Runtime.evaluate" {
                    // Return mutated guard indicating stale element
                    let resp = json!({
                        "id": id,
                        "result": {
                            "result": {
                                "value": [["key-mutated"], ["guard-mutated"]]
                            }
                        }
                    });
                    ws.send(Message::Text(resp.to_string().into()))
                        .await
                        .expect("send");
                }
            }
        }
    });

    let client = Arc::new(CdpClient::connect(&ws_url).await.expect("connect"));
    let mut browser = Browser::new(client, Some("sess-1".into()));

    let page = create_sample_page();
    let action = page.actions[0].clone();

    let result = browser.act(&action, &page, Some("text")).await;
    match result {
        Err(BrowserError::StalePage(msg)) => {
            assert!(msg.contains("Page changed since this decision"));
        }
        other => panic!("Expected BrowserError::StalePage, got {:?}", other),
    }
}

#[tokio::test]
async fn test_agent_step_done() {
    // 1. Mock TypeSafe server returning DONE
    let mock_typesafe = MockServer::start().await;
    let typesafe_body = json!({
        "model": "action",
        "answers": {
            "operation": {
                "choice": "DONE",
                "confidence": 1.0,
                "probabilities": {
                    "DONE": 1.0,
                    "WAIT": 0.0
                }
            }
        }
    });

    Mock::given(method("POST"))
        .and(path("/v1/systemone"))
        .respond_with(ResponseTemplate::new(200).set_body_json(typesafe_body))
        .mount(&mock_typesafe)
        .await;

    // 2. Mock CDP WebSocket server
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr: SocketAddr = listener.local_addr().expect("addr");
    let ws_url = format!("ws://{}", addr);

    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept");
        let mut ws = tokio_tungstenite::accept_async(stream).await.expect("ws");

        while let Some(msg) = ws.next().await {
            let msg = msg.expect("msg");
            if let Message::Text(text) = msg {
                let req: serde_json::Value = serde_json::from_str(&text).expect("json");
                let id = req["id"].as_u64().expect("id");
                let method = req["method"].as_str().unwrap_or("");

                if method == "Runtime.evaluate" {
                    let snapshot_result = json!({
                        "url": "https://example.com/done",
                        "title": "Done Page",
                        "text": "Success confirmation",
                        "scroll": [0, 800],
                        "actions": [],
                        "marker": ["marker-done"],
                        "page_key": ["page-done"],
                        "guards": {},
                        "omitted_actions": 0
                    });

                    let resp = json!({
                        "id": id,
                        "result": {
                            "result": {
                                "value": snapshot_result
                            }
                        }
                    });
                    ws.send(Message::Text(resp.to_string().into()))
                        .await
                        .expect("send");
                } else if method == "Page.captureScreenshot" {
                    let resp = json!({
                        "id": id,
                        "result": {"data": "shot"}
                    });
                    ws.send(Message::Text(resp.to_string().into()))
                        .await
                        .expect("send");
                }
            }
        }
    });

    let client = Arc::new(CdpClient::connect(&ws_url).await.expect("connect"));
    let mut browser = Browser::new(client, Some("sess-1".into()));

    let config = AgentConfig {
        typesafe_base_url: Some(mock_typesafe.uri()),
        ..Default::default()
    };

    let mut history = Vec::new();
    let decision = Agent::step(&mut browser, "Verify completion", &mut history, &config)
        .await
        .expect("step should succeed")
        .expect("step returned decision");

    assert_eq!(decision.operation, "DONE");
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].operation, "DONE");
}

#[tokio::test]
async fn test_agent_run_reaches_done() {
    let mock_typesafe = MockServer::start().await;
    let typesafe_body = json!({
        "model": "action",
        "answers": {
            "operation": {
                "choice": "DONE",
                "confidence": 1.0,
                "probabilities": {
                    "DONE": 1.0,
                    "WAIT": 0.0
                }
            }
        }
    });

    Mock::given(method("POST"))
        .and(path("/v1/systemone"))
        .respond_with(ResponseTemplate::new(200).set_body_json(typesafe_body))
        .mount(&mock_typesafe)
        .await;

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr: SocketAddr = listener.local_addr().expect("addr");
    let ws_url = format!("ws://{}", addr);

    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept");
        let mut ws = tokio_tungstenite::accept_async(stream).await.expect("ws");

        while let Some(msg) = ws.next().await {
            let msg = msg.expect("msg");
            if let Message::Text(text) = msg {
                let req: serde_json::Value = serde_json::from_str(&text).expect("json");
                let id = req["id"].as_u64().expect("id");
                let method = req["method"].as_str().unwrap_or("");

                if method == "Runtime.evaluate" {
                    let snapshot_result = json!({
                        "url": "https://example.com/done",
                        "title": "Done",
                        "text": "Success",
                        "scroll": [0, 800],
                        "actions": [],
                        "marker": ["marker"],
                        "page_key": ["key"],
                        "guards": {},
                        "omitted_actions": 0
                    });

                    let resp = json!({
                        "id": id,
                        "result": {
                            "result": {
                                "value": snapshot_result
                            }
                        }
                    });
                    ws.send(Message::Text(resp.to_string().into()))
                        .await
                        .expect("send");
                } else if method == "Page.captureScreenshot" {
                    let resp = json!({
                        "id": id,
                        "result": {"data": "shot"}
                    });
                    ws.send(Message::Text(resp.to_string().into()))
                        .await
                        .expect("send");
                }
            }
        }
    });

    let client = Arc::new(CdpClient::connect(&ws_url).await.expect("connect"));
    let mut browser = Browser::new(client, Some("sess-1".into()));

    let config = AgentConfig {
        typesafe_base_url: Some(mock_typesafe.uri()),
        max_steps: 5,
        ..Default::default()
    };

    let decisions = Agent::run(&mut browser, "Check done", &config)
        .await
        .expect("run should reach done");

    assert_eq!(decisions.len(), 1);
    assert_eq!(decisions[0].operation, "DONE");
}

#[tokio::test]
async fn test_agent_run_max_steps_budget() {
    let mock_typesafe = MockServer::start().await;
    let typesafe_body = json!({
        "model": "action",
        "answers": {
            "operation": {
                "choice": "WAIT",
                "confidence": 1.0,
                "probabilities": {
                    "WAIT": 1.0,
                    "DONE": 0.0
                }
            }
        }
    });

    Mock::given(method("POST"))
        .and(path("/v1/systemone"))
        .respond_with(ResponseTemplate::new(200).set_body_json(typesafe_body))
        .mount(&mock_typesafe)
        .await;

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr: SocketAddr = listener.local_addr().expect("addr");
    let ws_url = format!("ws://{}", addr);

    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept");
        let mut ws = tokio_tungstenite::accept_async(stream).await.expect("ws");

        while let Some(msg) = ws.next().await {
            let msg = msg.expect("msg");
            if let Message::Text(text) = msg {
                let req: serde_json::Value = serde_json::from_str(&text).expect("json");
                let id = req["id"].as_u64().expect("id");
                let method = req["method"].as_str().unwrap_or("");

                if method == "Runtime.evaluate" {
                    let expr = req["params"]["expression"].as_str().unwrap_or("");
                    let val = if expr.contains("return state?.marker") {
                        json!(["marker"])
                    } else {
                        json!({
                            "url": "https://example.com/waiting",
                            "title": "Waiting",
                            "text": "Waiting for results...",
                            "scroll": [0, 800],
                            "actions": [
                                {
                                    "id": "wait",
                                    "kind": "wait",
                                    "label": "Wait for page to update"
                                }
                            ],
                            "marker": ["marker"],
                            "page_key": ["key"],
                            "guards": {},
                            "omitted_actions": 0
                        })
                    };

                    let resp = json!({
                        "id": id,
                        "result": {
                            "result": {
                                "value": val
                            }
                        }
                    });
                    ws.send(Message::Text(resp.to_string().into()))
                        .await
                        .expect("send");
                } else if method == "Page.captureScreenshot" {
                    let resp = json!({
                        "id": id,
                        "result": {"data": "shot"}
                    });
                    ws.send(Message::Text(resp.to_string().into()))
                        .await
                        .expect("send");
                }
            }
        }
    });

    let client = Arc::new(CdpClient::connect(&ws_url).await.expect("connect"));
    let mut browser = Browser::new(client, Some("sess-1".into()));

    let config = AgentConfig {
        typesafe_base_url: Some(mock_typesafe.uri()),
        max_steps: 2,
        ..Default::default()
    };

    let result = Agent::run(&mut browser, "Wait until budget", &config).await;
    match result {
        Err(AgentError::MaxStepsExceeded) => {}
        other => panic!("Expected AgentError::MaxStepsExceeded, got {:?}", other),
    }
}

#[tokio::test]
async fn test_browser_act_scroll_mock() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr: SocketAddr = listener.local_addr().expect("addr");
    let ws_url = format!("ws://{}", addr);

    let scrolled_delta = Arc::new(Mutex::new(None));
    let scrolled_delta_clone = Arc::clone(&scrolled_delta);

    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept");
        let mut ws = tokio_tungstenite::accept_async(stream).await.expect("ws");

        while let Some(msg) = ws.next().await {
            let msg = msg.expect("msg");
            if let Message::Text(text) = msg {
                let req: serde_json::Value = serde_json::from_str(&text).expect("json");
                let id = req["id"].as_u64().expect("id");
                let method = req["method"].as_str().unwrap_or("");

                if method == "Runtime.evaluate" {
                    let resp = json!({
                        "id": id,
                        "result": {
                            "result": {
                                "value": ["marker-val"]
                            }
                        }
                    });
                    ws.send(Message::Text(resp.to_string().into()))
                        .await
                        .expect("send");
                } else if method == "Input.dispatchMouseEvent" {
                    let delta_y = req["params"]["deltaY"].as_i64();
                    *scrolled_delta_clone.lock().await = delta_y;
                    let resp = json!({"id": id, "result": {}});
                    ws.send(Message::Text(resp.to_string().into()))
                        .await
                        .expect("send");
                }
            }
        }
    });

    let client = Arc::new(CdpClient::connect(&ws_url).await.expect("connect"));
    let mut browser = Browser::new(client, Some("sess-1".into()));

    let page = create_sample_page();
    let scroll_action = page.actions[2].clone(); // scroll_down

    let res = browser
        .act(&scroll_action, &page, None)
        .await
        .expect("act scroll");
    assert_eq!(res["executed"], "scroll_down");
    assert_eq!(*scrolled_delta.lock().await, Some(560));
}

#[tokio::test]
async fn test_browser_act_wait_mock() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr: SocketAddr = listener.local_addr().expect("addr");
    let ws_url = format!("ws://{}", addr);

    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept");
        let mut ws = tokio_tungstenite::accept_async(stream).await.expect("ws");

        while let Some(msg) = ws.next().await {
            let msg = msg.expect("msg");
            if let Message::Text(text) = msg {
                let req: serde_json::Value = serde_json::from_str(&text).expect("json");
                let id = req["id"].as_u64().expect("id");
                let method = req["method"].as_str().unwrap_or("");

                if method == "Runtime.evaluate" {
                    let resp = json!({
                        "id": id,
                        "result": {
                            "result": {
                                "value": ["marker-val"]
                            }
                        }
                    });
                    ws.send(Message::Text(resp.to_string().into()))
                        .await
                        .expect("send");
                }
            }
        }
    });

    let client = Arc::new(CdpClient::connect(&ws_url).await.expect("connect"));
    let mut browser = Browser::new(client, Some("sess-1".into()));

    let page = create_sample_page();
    let wait_action = page.actions[3].clone(); // wait

    let res = browser
        .act(&wait_action, &page, None)
        .await
        .expect("act wait");
    assert_eq!(res["executed"], "wait");
}

#[tokio::test]
async fn test_browser_open_mock() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr: SocketAddr = listener.local_addr().expect("addr");
    let ws_url = format!("ws://{}", addr);

    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept");
        let mut ws = tokio_tungstenite::accept_async(stream).await.expect("ws");

        while let Some(msg) = ws.next().await {
            let msg = msg.expect("msg");
            if let Message::Text(text) = msg {
                let req: serde_json::Value = serde_json::from_str(&text).expect("json");
                let id = req["id"].as_u64().expect("id");
                let method = req["method"].as_str().unwrap_or("");

                let resp = match method {
                    "Target.createTarget" => json!({
                        "id": id,
                        "result": {"targetId": "target-mock-99"}
                    }),
                    "Target.attachToTarget" => json!({
                        "id": id,
                        "result": {"sessionId": "sess-mock-99"}
                    }),
                    "Emulation.setDeviceMetricsOverride"
                    | "Emulation.setFocusEmulationEnabled"
                    | "Page.navigate" => json!({
                        "id": id,
                        "result": {}
                    }),
                    "Runtime.evaluate" => json!({
                        "id": id,
                        "result": {
                            "result": {
                                "value": "complete"
                            }
                        }
                    }),
                    _ => json!({"id": id, "result": {}}),
                };

                ws.send(Message::Text(resp.to_string().into()))
                    .await
                    .expect("send");
            }
        }
    });

    let client = Arc::new(CdpClient::connect(&ws_url).await.expect("connect"));
    let mut browser = Browser::new(client, None);

    browser
        .open("https://example.com/hello")
        .await
        .expect("open");
    assert_eq!(browser.target_id, Some("target-mock-99".to_string()));
    assert_eq!(browser.session_id, Some("sess-mock-99".to_string()));
}

#[tokio::test]
async fn test_browser_close_mock() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr: SocketAddr = listener.local_addr().expect("addr");
    let ws_url = format!("ws://{}", addr);

    let closed_target = Arc::new(Mutex::new(None));
    let closed_target_clone = Arc::clone(&closed_target);

    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept");
        let mut ws = tokio_tungstenite::accept_async(stream).await.expect("ws");

        while let Some(msg) = ws.next().await {
            let msg = msg.expect("msg");
            if let Message::Text(text) = msg {
                let req: serde_json::Value = serde_json::from_str(&text).expect("json");
                let id = req["id"].as_u64().expect("id");
                let method = req["method"].as_str().unwrap_or("");

                if method == "Target.closeTarget" {
                    let tid = req["params"]["targetId"].as_str().map(|s| s.to_string());
                    *closed_target_clone.lock().await = tid;
                    let resp = json!({"id": id, "result": {}});
                    ws.send(Message::Text(resp.to_string().into()))
                        .await
                        .expect("send");
                }
            }
        }
    });

    let client = Arc::new(CdpClient::connect(&ws_url).await.expect("connect"));
    let mut browser = Browser::new(client, Some("sess-1".into()));
    browser.target_id = Some("target-to-close".into());

    browser.close().await.expect("close");
    assert_eq!(browser.target_id, None);
    assert_eq!(browser.session_id, None);
    assert_eq!(
        *closed_target.lock().await,
        Some("target-to-close".to_string())
    );
}

#[tokio::test]
async fn test_agent_step_type_text_with_llm() {
    // 1. Mock TypeSafe server returning TYPE_TEXT with target e1
    let mock_typesafe = MockServer::start().await;
    let typesafe_body = json!({
        "model": "action",
        "answers": {
            "operation": {
                "choice": "TYPE_TEXT",
                "confidence": 0.98,
                "probabilities": {
                    "TYPE_TEXT": 0.98,
                    "WAIT": 0.02,
                    "DONE": 0.0
                }
            },
            "type_text_target": {
                "choice": "e1",
                "confidence": 1.0,
                "probabilities": {
                    "e1": 1.0
                }
            }
        }
    });

    Mock::given(method("POST"))
        .and(path("/v1/systemone"))
        .respond_with(ResponseTemplate::new(200).set_body_json(typesafe_body))
        .mount(&mock_typesafe)
        .await;

    // 2. Mock LLM server returning completion for field_text
    let mock_llm = MockServer::start().await;
    let llm_body = json!({
        "choices": [
            {
                "message": {
                    "content": "{\"text\": \"Rust in Action\"}"
                }
            }
        ]
    });

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(llm_body))
        .mount(&mock_llm)
        .await;

    // 3. Mock CDP WebSocket server
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr: SocketAddr = listener.local_addr().expect("addr");
    let ws_url = format!("ws://{}", addr);

    let inserted_text = Arc::new(Mutex::new(String::new()));
    let inserted_text_clone = Arc::clone(&inserted_text);

    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept");
        let mut ws = tokio_tungstenite::accept_async(stream).await.expect("ws");

        while let Some(msg) = ws.next().await {
            let msg = msg.expect("msg");
            if let Message::Text(text) = msg {
                let req: serde_json::Value = serde_json::from_str(&text).expect("json");
                let id = req["id"].as_u64().expect("id");
                let method = req["method"].as_str().unwrap_or("");

                if method == "Runtime.evaluate" {
                    let expr = req["params"]["expression"].as_str().unwrap_or("");
                    if expr.contains("c.pageKey()") {
                        let resp = json!({
                            "id": id,
                            "result": {
                                "result": {
                                    "value": [["key-1"], ["guard-101"]]
                                }
                            }
                        });
                        ws.send(Message::Text(resp.to_string().into()))
                            .await
                            .expect("send");
                    } else if expr.contains("document.elementFromPoint") {
                        let resp = json!({
                            "id": id,
                            "result": {
                                "result": {
                                    "value": {"x": 120.0, "y": 120.0}
                                }
                            }
                        });
                        ws.send(Message::Text(resp.to_string().into()))
                            .await
                            .expect("send");
                    } else {
                        // Snapshot
                        let snapshot_result = json!({
                            "url": "https://example.com/form",
                            "title": "Search",
                            "text": "Search box",
                            "scroll": [0, 800],
                            "actions": [
                                {
                                    "id": "e1",
                                    "kind": "fill",
                                    "label": "Search query",
                                    "role": "textbox",
                                    "value": "",
                                    "node": 101
                                }
                            ],
                            "marker": ["marker"],
                            "page_key": ["key-1"],
                            "guards": {"101": ["guard-101"]},
                            "omitted_actions": 0
                        });

                        let resp = json!({
                            "id": id,
                            "result": {
                                "result": {
                                    "value": snapshot_result
                                }
                            }
                        });
                        ws.send(Message::Text(resp.to_string().into()))
                            .await
                            .expect("send");
                    }
                } else if method == "Input.dispatchMouseEvent" || method == "Input.dispatchKeyEvent"
                {
                    let resp = json!({"id": id, "result": {}});
                    ws.send(Message::Text(resp.to_string().into()))
                        .await
                        .expect("send");
                } else if method == "Input.insertText" {
                    let txt = req["params"]["text"].as_str().unwrap_or("").to_string();
                    *inserted_text_clone.lock().await = txt;
                    let resp = json!({"id": id, "result": {}});
                    ws.send(Message::Text(resp.to_string().into()))
                        .await
                        .expect("send");
                } else if method == "Page.captureScreenshot" {
                    let resp = json!({
                        "id": id,
                        "result": {"data": "shot"}
                    });
                    ws.send(Message::Text(resp.to_string().into()))
                        .await
                        .expect("send");
                }
            }
        }
    });

    let client = Arc::new(CdpClient::connect(&ws_url).await.expect("connect"));
    let mut browser = Browser::new(client, Some("sess-1".into()));

    let config = AgentConfig {
        typesafe_base_url: Some(mock_typesafe.uri()),
        text_model_base_url: Some(mock_llm.uri()),
        text_model_api_key: Some("mock-key".into()),
        ..Default::default()
    };

    let mut history = Vec::new();
    let decision = Agent::step(&mut browser, "Search for Rust books", &mut history, &config)
        .await
        .expect("step")
        .expect("decision");

    assert_eq!(decision.operation, "TYPE_TEXT");
    assert_eq!(decision.target, Some("e1".to_string()));
    assert_eq!(decision.text, Some("Rust in Action".to_string()));
    assert_eq!(*inserted_text.lock().await, "Rust in Action");
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].text, Some("Rust in Action".to_string()));
}
