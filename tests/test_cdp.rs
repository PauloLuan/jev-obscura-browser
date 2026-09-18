use futures_util::{SinkExt, StreamExt};
use jev_obscura_browser::cdp::{
    ensure_obscura, format_cdp_request, parse_cdp_response, CdpClient, CdpError,
};
use serde_json::json;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

#[test]
fn test_cdp_request_formatting() {
    let req = format_cdp_request(
        42,
        "Target.createTarget",
        json!({"url": "about:blank"}),
        None,
    );
    assert_eq!(req["id"], 42);
    assert_eq!(req["method"], "Target.createTarget");
    assert_eq!(req["params"]["url"], "about:blank");
    assert!(req.get("sessionId").is_none());
}

#[test]
fn test_cdp_request_with_session() {
    let req = format_cdp_request(
        43,
        "Runtime.evaluate",
        json!({"expression": "1+1"}),
        Some("sess-1"),
    );
    assert_eq!(req["id"], 43);
    assert_eq!(req["method"], "Runtime.evaluate");
    assert_eq!(req["params"]["expression"], "1+1");
    assert_eq!(req["sessionId"], "sess-1");
}

#[test]
fn test_cdp_error_parsing() {
    let raw = json!({
        "id": 44,
        "error": {"code": -32000, "message": "Evaluation failed"}
    });
    let result = parse_cdp_response(raw);
    match result {
        Err(CdpError::ProtocolError { code, message }) => {
            assert_eq!(code, -32000);
            assert_eq!(message, "Evaluation failed");
        }
        other => panic!("Expected ProtocolError, got {:?}", other),
    }
}

#[test]
fn test_cdp_success_parsing() {
    let raw = json!({
        "id": 45,
        "result": {"targetId": "target-123"}
    });
    let result = parse_cdp_response(raw).expect("Expected success");
    assert_eq!(result["targetId"], "target-123");
}

#[test]
fn test_cdp_invalid_responses() {
    // Missing ID
    let no_id = json!({"result": {}});
    assert!(matches!(
        parse_cdp_response(no_id),
        Err(CdpError::ProtocolError { .. })
    ));

    // Non-object
    let non_obj = json!("not an object");
    assert!(matches!(
        parse_cdp_response(non_obj),
        Err(CdpError::ProtocolError { .. })
    ));

    // Missing both result and error
    let empty_obj = json!({"id": 10});
    assert!(matches!(
        parse_cdp_response(empty_obj),
        Err(CdpError::ProtocolError { .. })
    ));
}

#[tokio::test]
async fn test_cdp_client_roundtrip() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr: SocketAddr = listener.local_addr().expect("local addr");
    let ws_url = format!("ws://{}", addr);

    // Spawn mock WebSocket server
    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept connection");
        let mut ws = tokio_tungstenite::accept_async(stream)
            .await
            .expect("accept ws");

        while let Some(msg) = ws.next().await {
            let msg = msg.expect("valid msg");
            if let Message::Text(text) = msg {
                let req: serde_json::Value = serde_json::from_str(&text).expect("valid json");
                let id = req["id"].as_u64().expect("id is u64");
                let method = req["method"].as_str().unwrap_or("");

                let resp = json!({
                    "id": id,
                    "result": {
                        "echoed_method": method,
                        "success": true
                    }
                });
                ws.send(Message::Text(resp.to_string().into()))
                    .await
                    .expect("send resp");
            }
        }
    });

    let client = CdpClient::connect(&ws_url)
        .await
        .expect("connect to mock cdp");
    let res = client
        .send_request("Target.getTargets", json!({}), None)
        .await
        .expect("send_request succeeded");

    assert_eq!(res["echoed_method"], "Target.getTargets");
    assert_eq!(res["success"], true);

    // Test call alias
    let res2 = client
        .call("Browser.getVersion", json!({}), Some("session-abc"))
        .await
        .expect("call succeeded");
    assert_eq!(res2["echoed_method"], "Browser.getVersion");
    assert_eq!(res2["success"], true);
}

#[tokio::test]
async fn test_cdp_client_concurrent_multiplexing_and_events() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("local addr");
    let ws_url = format!("ws://{}", addr);

    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept connection");
        let mut ws = tokio_tungstenite::accept_async(stream)
            .await
            .expect("accept ws");

        let mut received = Vec::new();
        // Receive 3 requests
        for _ in 0..3 {
            if let Some(Ok(Message::Text(text))) = ws.next().await {
                let val: serde_json::Value = serde_json::from_str(&text).expect("json");
                received.push(val);
            }
        }

        // Send an unprompted CDP event before responses
        let event = json!({
            "method": "Target.targetCreated",
            "params": {"targetInfo": {"targetId": "event-1"}}
        });
        ws.send(Message::Text(event.to_string().into()))
            .await
            .expect("send event");

        // Send responses in reverse order
        for req in received.into_iter().rev() {
            let id = req["id"].as_u64().unwrap();
            let resp = json!({
                "id": id,
                "result": {"answered_id": id}
            });
            ws.send(Message::Text(resp.to_string().into()))
                .await
                .expect("send resp");
        }
    });

    let client = std::sync::Arc::new(CdpClient::connect(&ws_url).await.expect("connect"));

    let c1 = client.clone();
    let c2 = client.clone();
    let c3 = client.clone();

    let (r1, r2, r3) = tokio::join!(
        tokio::spawn(async move { c1.send_request("req1", json!({}), None).await }),
        tokio::spawn(async move { c2.send_request("req2", json!({}), None).await }),
        tokio::spawn(async move { c3.send_request("req3", json!({}), None).await }),
    );

    let val1 = r1.unwrap().expect("r1 ok");
    let val2 = r2.unwrap().expect("r2 ok");
    let val3 = r3.unwrap().expect("r3 ok");

    assert!(val1["answered_id"].as_u64().is_some());
    assert!(val2["answered_id"].as_u64().is_some());
    assert!(val3["answered_id"].as_u64().is_some());
    assert_ne!(val1["answered_id"], val2["answered_id"]);
    assert_ne!(val2["answered_id"], val3["answered_id"]);
}

#[tokio::test]
async fn test_cdp_client_connection_closed() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("local addr");
    let ws_url = format!("ws://{}", addr);

    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept connection");
        let mut ws = tokio_tungstenite::accept_async(stream)
            .await
            .expect("accept ws");
        // Read one request, then abruptly close
        let _ = ws.next().await;
        let _ = ws.close(None).await;
    });

    let client = CdpClient::connect(&ws_url).await.expect("connect");
    let res = client.send_request("Test.hang", json!({}), None).await;
    assert!(matches!(res, Err(CdpError::ConnectionClosed)));
}

#[tokio::test]
async fn test_cdp_client_timeout() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("local addr");
    let ws_url = format!("ws://{}", addr);

    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept connection");
        let mut ws = tokio_tungstenite::accept_async(stream)
            .await
            .expect("accept ws");
        // Read requests but never respond
        while ws.next().await.is_some() {}
    });

    let client = CdpClient::connect(&ws_url).await.expect("connect");
    let res = client
        .send_request_timeout("Test.timeout", json!({}), None, Duration::from_millis(50))
        .await;
    assert!(matches!(res, Err(CdpError::Timeout)));
}

#[tokio::test]
async fn test_supervisor_already_running() {
    // Open a mock TCP listener
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let port = listener.local_addr().expect("local addr").port();
    let url_arg = format!("127.0.0.1:{}", port);

    let res = ensure_obscura(Some(&url_arg)).await;
    assert!(
        res.is_ok(),
        "Expected success for already running port, got {:?}",
        res
    );
    let ws_url = res.unwrap();
    assert!(ws_url.contains(&format!(":{}", port)));
}

static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[tokio::test]
async fn test_supervisor_not_found() {
    let _lock = ENV_LOCK.lock().await;

    // Pick an unlikely port on loopback that has nothing listening
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().unwrap().port();
    drop(listener); // Free the port so it is closed

    // Ensure OBSCURA_BIN is unset and PATH does not contain obscura for this test
    let saved_obscura_bin = std::env::var("OBSCURA_BIN").ok();
    std::env::remove_var("OBSCURA_BIN");

    let saved_path = std::env::var("PATH").ok();
    std::env::set_var("PATH", "/nonexistent_dir_for_test");

    let url_arg = format!("127.0.0.1:{}", port);
    let res = ensure_obscura(Some(&url_arg)).await;

    // Restore env vars
    if let Some(v) = saved_obscura_bin {
        std::env::set_var("OBSCURA_BIN", v);
    }
    if let Some(v) = saved_path {
        std::env::set_var("PATH", v);
    }

    match res {
        Err(CdpError::SupervisorError(msg)) => {
            assert!(
                msg.contains("Obscura browser binary not found"),
                "Expected binary not found message, got: {}",
                msg
            );
            assert!(
                msg.contains("https://obscura.sh/"),
                "Expected obscura.sh URL in message, got: {}",
                msg
            );
        }
        other => panic!("Expected SupervisorError, got {:?}", other),
    }
}

#[tokio::test]
async fn test_supervisor_auto_spawn() {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;

    let _lock = ENV_LOCK.lock().await;

    // Pick an ephemeral port
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().unwrap().port();
    drop(listener); // Free it

    // Create a temporary mock obscura binary
    let tmp_dir = std::env::temp_dir();
    let mock_bin = tmp_dir.join(format!("mock_obscura_{}", port));

    let script = r#"#!/bin/sh
port="9222"
while [ $# -gt 0 ]; do
  case "$1" in
    --port)
      port="$2"
      shift 2
      ;;
    *)
      shift
      ;;
  esac
done
/usr/bin/python3 -c "import socket, time; s = socket.socket(); s.bind(('127.0.0.1', int('$port'))); s.listen(1); time.sleep(3)"
"#;

    let mut f = std::fs::File::create(&mock_bin).expect("create mock bin");
    f.write_all(script.as_bytes()).expect("write mock bin");
    f.flush().expect("flush mock bin");
    drop(f);

    let perms = std::fs::Permissions::from_mode(0o755);
    std::fs::set_permissions(&mock_bin, perms).expect("set perms");

    let saved_obscura_bin = std::env::var("OBSCURA_BIN").ok();
    std::env::set_var("OBSCURA_BIN", mock_bin.to_str().unwrap());

    let saved_path = std::env::var("PATH").ok();
    std::env::set_var("PATH", "/usr/bin:/bin");

    let url_arg = format!("127.0.0.1:{}", port);
    let res = ensure_obscura(Some(&url_arg)).await;

    // Cleanup
    if let Some(v) = saved_obscura_bin {
        std::env::set_var("OBSCURA_BIN", v);
    } else {
        std::env::remove_var("OBSCURA_BIN");
    }
    if let Some(v) = saved_path {
        std::env::set_var("PATH", v);
    }
    let _ = std::fs::remove_file(&mock_bin);

    assert!(res.is_ok(), "Expected auto-spawn success, got {:?}", res);
    let ws_url = res.unwrap();
    assert!(ws_url.contains(&format!(":{}", port)));
}

#[tokio::test]
async fn test_supervisor_spawn_from_path() {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;

    let _lock = ENV_LOCK.lock().await;

    // Pick an ephemeral port
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    // Create a temporary directory in temp
    let tmp_dir = std::env::temp_dir().join(format!("obscura_path_test_{}", port));
    std::fs::create_dir_all(&tmp_dir).expect("create tmp dir");
    let mock_bin = tmp_dir.join("obscura");

    let script = r#"#!/bin/sh
port="9222"
while [ $# -gt 0 ]; do
  case "$1" in
    --port)
      port="$2"
      shift 2
      ;;
    *)
      shift
      ;;
  esac
done
/usr/bin/python3 -c "import socket, time; s = socket.socket(); s.bind(('127.0.0.1', int('$port'))); s.listen(1); time.sleep(3)"
"#;

    let mut f = std::fs::File::create(&mock_bin).expect("create mock bin");
    f.write_all(script.as_bytes()).expect("write mock bin");
    f.flush().expect("flush mock bin");
    drop(f);

    let perms = std::fs::Permissions::from_mode(0o755);
    std::fs::set_permissions(&mock_bin, perms).expect("set perms");

    let saved_obscura_bin = std::env::var("OBSCURA_BIN").ok();
    std::env::remove_var("OBSCURA_BIN");

    let saved_path = std::env::var("PATH").ok();
    std::env::set_var("PATH", format!("{}:/usr/bin:/bin", tmp_dir.display()));

    let url_arg = format!("127.0.0.1:{}", port);
    let res = ensure_obscura(Some(&url_arg)).await;

    // Cleanup
    if let Some(v) = saved_obscura_bin {
        std::env::set_var("OBSCURA_BIN", v);
    }
    if let Some(v) = saved_path {
        std::env::set_var("PATH", v);
    }
    let _ = std::fs::remove_dir_all(&tmp_dir);

    assert!(res.is_ok(), "Expected path-spawn success, got {:?}", res);
    let ws_url = res.unwrap();
    assert!(ws_url.contains(&format!(":{}", port)));
}
