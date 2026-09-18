use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use clap::Parser;
use http_body_util::BodyExt;
use jev_obscura_browser::demo::{create_router, AppState, Cli, Commands};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::RwLock;
use tower::ServiceExt;

#[tokio::test]
async fn test_index_route() {
    let state = Arc::new(RwLock::new(AppState::default()));
    let app = create_router(state);

    let req = Request::builder()
        .uri("/")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let content_type = res
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    assert!(
        content_type.contains("text/html"),
        "Content-Type should be text/html, got: {}",
        content_type
    );

    let body_bytes = res.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8_lossy(&body_bytes);
    assert!(
        body_str.contains("Jev Obscura Browser") || body_str.contains("obscura"),
        "Body did not contain expected branding: {}",
        body_str
    );
}

#[tokio::test]
async fn test_api_state_empty() {
    let state = Arc::new(RwLock::new(AppState::default()));
    let app = create_router(state);

    let req = Request::builder()
        .uri("/api/state")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = res.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(json["status"], "idle");
    assert_eq!(json["goal"], "");
    assert!(json["page"].is_null());
    assert!(json["decision"].is_null());
    assert!(json["history"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_api_start_and_reset() {
    let state = Arc::new(RwLock::new(AppState::default()));
    let app = create_router(state);

    // Test POST /api/start
    let start_payload = json!({
        "goal": "Find flights from Zurich to London",
        "scenario": "flights"
    });
    let req = Request::builder()
        .uri("/api/start")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_vec(&start_payload).unwrap()))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = res.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(json["goal"], "Find flights from Zurich to London");
    assert_eq!(json["scenario"], "flights");
    assert_eq!(json["status"], "ready");

    // Test POST /api/reset
    let reset_payload = json!({
        "goal": "Find a stay in Lisbon",
        "scenario": "travel"
    });
    let req = Request::builder()
        .uri("/api/reset")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_vec(&reset_payload).unwrap()))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = res.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(json["goal"], "Find a stay in Lisbon");
    assert_eq!(json["scenario"], "travel");
    assert_eq!(json["status"], "ready");
}

#[tokio::test]
async fn test_api_step() {
    let state = Arc::new(RwLock::new(AppState::default()));
    let app = create_router(state);

    // First start a task
    let start_payload = json!({
        "goal": "Test stepping task"
    });
    let start_req = Request::builder()
        .uri("/api/start")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_vec(&start_payload).unwrap()))
        .unwrap();
    let _ = app.clone().oneshot(start_req).await.unwrap();

    // Now call POST /api/step
    let step_req = Request::builder()
        .uri("/api/step")
        .method("POST")
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(step_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = res.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert!(!json["history"].as_array().unwrap().is_empty());
    assert!(json["decision"].is_object() || !json["decision"].is_null());
}

#[tokio::test]
async fn test_cli_arg_parsing() {
    // 1. serve default port
    let cli = Cli::try_parse_from(["jev-obscura", "serve"]).unwrap();
    assert_eq!(cli.command, Commands::Serve { port: 8766 });

    // 2. serve custom port
    let cli = Cli::try_parse_from(["jev-obscura", "serve", "--port", "9191"]).unwrap();
    assert_eq!(cli.command, Commands::Serve { port: 9191 });

    // 3. run subcommand
    let cli = Cli::try_parse_from([
        "jev-obscura",
        "run",
        "--url",
        "https://example.com",
        "--goal",
        "Search flights",
        "--max-steps",
        "15",
    ])
    .unwrap();
    assert_eq!(
        cli.command,
        Commands::Run {
            url: "https://example.com".to_string(),
            goal: "Search flights".to_string(),
            max_steps: 15,
        }
    );

    // 4. check subcommand
    let cli = Cli::try_parse_from(["jev-obscura", "check"]).unwrap();
    assert_eq!(cli.command, Commands::Check);
}

#[tokio::test]
async fn test_api_predict_and_act_and_tick() {
    let state = Arc::new(RwLock::new(AppState::default()));
    let app = create_router(state);

    let start_payload = json!({
        "goal": "Test predict and act"
    });
    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/start")
                .method("POST")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&start_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // POST /api/predict
    let predict_req = Request::builder()
        .uri("/api/predict")
        .method("POST")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(predict_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body_bytes = res.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(json["status"], "predicted");
    assert!(json["decision"].is_object() || !json["decision"].is_null());

    // POST /api/act
    let act_req = Request::builder()
        .uri("/api/act")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::to_vec(&json!({"fingerprint": "test_fp"})).unwrap(),
        ))
        .unwrap();
    let res = app.clone().oneshot(act_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body_bytes = res.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(json["status"], "ready");
    assert!(!json["history"].as_array().unwrap().is_empty());

    // POST /api/tick
    let tick_req = Request::builder()
        .uri("/api/tick")
        .method("POST")
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(tick_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}
