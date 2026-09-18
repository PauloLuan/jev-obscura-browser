use std::sync::Arc;

use axum::extract::{Json, State};
use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::Router;
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::agent::{Agent, AgentConfig};
use crate::browser::Browser;
use crate::types::{Action, Decision, PageState};

/// Transforms observed page actions into frontend element objects.
pub fn actions_to_elements(actions: &[Action]) -> Vec<serde_json::Value> {
    actions
        .iter()
        .map(|a| {
            serde_json::json!({
                "index": a.id,
                "label": a.label,
                "role": a.role,
                "operations": [a.kind.to_uppercase()],
                "value": a.value,
                "checked": a.checked,
            })
        })
        .collect()
}

/// Thread-safe shared application state behind `Arc<RwLock<AppState>>`.
pub struct AppState {
    pub status: String,
    pub goal: String,
    pub url: Option<String>,
    pub scenario: Option<String>,
    pub page: Option<PageState>,
    pub elements: Vec<serde_json::Value>,
    pub decision: Option<Decision>,
    pub decisions: Vec<Decision>,
    pub history: Vec<Decision>,
    pub screenshot: Option<String>,
    pub text_model: String,
    pub plan: Vec<String>,
    pub plan_index: usize,
    pub elapsed_ms: u64,
    pub max_steps: usize,
    pub browser: Option<Browser>,
    pub config: AgentConfig,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            status: "idle".to_string(),
            goal: String::new(),
            url: None,
            scenario: None,
            page: None,
            elements: Vec::new(),
            decision: None,
            decisions: Vec::new(),
            history: Vec::new(),
            screenshot: None,
            text_model: "deepseek-chat".to_string(),
            plan: Vec::new(),
            plan_index: 0,
            elapsed_ms: 0,
            max_steps: 30,
            browser: None,
            config: AgentConfig::default(),
        }
    }
}

impl AppState {
    pub fn to_response(&self) -> StateResponse {
        StateResponse {
            status: self.status.clone(),
            goal: self.goal.clone(),
            url: self.url.clone(),
            scenario: self.scenario.clone(),
            page: self.page.clone(),
            elements: self.elements.clone(),
            decision: self.decision.clone(),
            decisions: self.decisions.clone(),
            history: self.history.clone(),
            screenshot: self
                .screenshot
                .clone()
                .or_else(|| self.page.as_ref().and_then(|p| p.screenshot.clone())),
            text_model: self.text_model.clone(),
            plan: self.plan.clone(),
            plan_index: self.plan_index,
            elapsed_ms: self.elapsed_ms,
            max_steps: self.max_steps,
        }
    }
}

/// JSON payload structure returned by state endpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateResponse {
    pub status: String,
    pub goal: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scenario: Option<String>,
    pub page: Option<PageState>,
    pub elements: Vec<serde_json::Value>,
    pub decision: Option<Decision>,
    pub decisions: Vec<Decision>,
    pub history: Vec<Decision>,
    pub screenshot: Option<String>,
    pub text_model: String,
    pub plan: Vec<String>,
    pub plan_index: usize,
    pub elapsed_ms: u64,
    pub max_steps: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct StartRequest {
    #[serde(default)]
    pub goal: String,
    pub url: Option<String>,
    pub scenario: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ActRequest {
    pub fingerprint: Option<String>,
}

#[derive(Parser, Debug)]
#[command(
    name = "jev-obscura",
    version,
    about = "Fast browser agent in Rust using Obscura and TypeSafe"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug, PartialEq)]
pub enum Commands {
    /// Starts web inspector server
    Serve {
        /// Port to listen on [default: 8766]
        #[arg(short, long, default_value_t = 8766)]
        port: u16,
    },
    /// Runs headless agent to completion in terminal
    Run {
        /// URL to open
        #[arg(short, long)]
        url: String,
        /// Task goal description
        #[arg(short, long)]
        goal: String,
        /// Maximum execution steps [default: 30]
        #[arg(long, default_value_t = 30)]
        max_steps: usize,
    },
    /// Probes Obscura on port 9222 and checks TypeSafe API environment variables
    Check,
}

async fn index_handler() -> impl IntoResponse {
    match tokio::fs::read_to_string("static/index.html").await {
        Ok(html) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
            html,
        )
            .into_response(),
        Err(_) => {
            let fallback = include_str!("../static/index.html");
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                fallback,
            )
                .into_response()
        }
    }
}

async fn get_state_handler(State(state): State<Arc<RwLock<AppState>>>) -> impl IntoResponse {
    let app = state.read().await;
    (StatusCode::OK, Json(app.to_response()))
}

async fn start_handler(
    State(state): State<Arc<RwLock<AppState>>>,
    Json(payload): Json<StartRequest>,
) -> impl IntoResponse {
    let mut app = state.write().await;
    app.goal = payload.goal;
    app.scenario = payload.scenario.clone();
    let url = payload
        .url
        .unwrap_or_else(|| match app.scenario.as_deref() {
            Some("flights") => "https://www.google.com/travel/flights".to_string(),
            Some("travel") => "http://127.0.0.1:8766/fixture.html#travel".to_string(),
            Some("research") => "http://127.0.0.1:8766/fixture.html#research".to_string(),
            _ => "about:blank".to_string(),
        });
    app.url = Some(url.clone());
    app.status = "ready".to_string();
    app.decision = None;
    app.decisions.clear();
    app.history.clear();
    app.elapsed_ms = 0;
    app.plan = if app.goal.is_empty() {
        vec![]
    } else {
        vec![app.goal.clone()]
    };
    app.plan_index = 0;

    let maybe_browser = app.browser.take();
    if let Some(mut browser) = maybe_browser {
        if browser.open(&url).await.is_ok() {
            if let Ok(page) = browser.observe(true).await {
                app.elements = actions_to_elements(&page.actions);
                app.screenshot = page.screenshot.clone();
                app.page = Some(page);
            }
        }
        app.browser = Some(browser);
    }

    (StatusCode::OK, Json(app.to_response()))
}

async fn step_handler(State(state): State<Arc<RwLock<AppState>>>) -> impl IntoResponse {
    let mut app = state.write().await;
    let maybe_browser = app.browser.take();
    if let Some(mut browser) = maybe_browser {
        let goal = app.goal.clone();
        let config = app.config.clone();
        match Agent::step(&mut browser, &goal, &mut app.history, &config).await {
            Ok(Some(decision)) => {
                let op = decision.operation.clone();
                app.decision = Some(decision.clone());
                app.decisions.push(decision);
                if op == "DONE" {
                    app.status = "done".to_string();
                } else if op == "BLOCKED" {
                    app.status = "blocked".to_string();
                } else {
                    app.status = "ready".to_string();
                }
                if let Ok(page) = browser.observe(true).await {
                    app.elements = actions_to_elements(&page.actions);
                    app.screenshot = page.screenshot.clone();
                    app.page = Some(page);
                }
            }
            Ok(None) => {
                app.status = "done".to_string();
            }
            Err(e) => {
                tracing::warn!("Step error: {:?}", e);
                app.status = "blocked".to_string();
            }
        }
        app.browser = Some(browser);
    } else {
        let decision = Decision {
            operation: "WAIT".to_string(),
            target: None,
            text: None,
        };
        app.decision = Some(decision.clone());
        app.decisions.push(decision.clone());
        app.history.push(decision);
        app.status = "ready".to_string();
    }
    (StatusCode::OK, Json(app.to_response()))
}

async fn predict_handler(State(state): State<Arc<RwLock<AppState>>>) -> impl IntoResponse {
    let mut app = state.write().await;
    let maybe_browser = app.browser.take();
    if let Some(mut browser) = maybe_browser {
        let http_client = reqwest::Client::new();
        let page_res = if let Some(ref p) = app.page {
            Ok(p.clone())
        } else {
            browser.observe(true).await
        };
        if let Ok(page) = page_res {
            app.elements = actions_to_elements(&page.actions);
            app.screenshot = page.screenshot.clone();
            app.page = Some(page.clone());

            let typesafe_url = app
                .config
                .typesafe_base_url
                .as_deref()
                .unwrap_or("https://api.typesafe.ai");
            let api_key = app.config.typesafe_api_key.as_deref().unwrap_or("");
            let model = app.config.typesafe_model.as_deref().unwrap_or("action");
            let history_vals: Vec<serde_json::Value> = app
                .history
                .iter()
                .map(|d| serde_json::to_value(d).unwrap_or_default())
                .collect();

            match crate::model::choose(
                &http_client,
                typesafe_url,
                api_key,
                model,
                &page,
                &app.goal,
                &history_vals,
            )
            .await
            {
                Ok(decision) => {
                    app.decision = Some(decision);
                    app.status = "predicted".to_string();
                }
                Err(e) => {
                    tracing::warn!("Predict error: {:?}", e);
                    app.status = "blocked".to_string();
                }
            }
        }
        app.browser = Some(browser);
    } else {
        let decision = Decision {
            operation: "CLICK".to_string(),
            target: Some("1".to_string()),
            text: None,
        };
        app.decision = Some(decision);
        app.status = "predicted".to_string();
    }
    (StatusCode::OK, Json(app.to_response()))
}

async fn act_handler(
    State(state): State<Arc<RwLock<AppState>>>,
    _body: Option<Json<ActRequest>>,
) -> impl IntoResponse {
    let mut app = state.write().await;
    if let Some(decision) = app.decision.clone() {
        let maybe_browser = app.browser.take();
        if let Some(mut browser) = maybe_browser {
            if let Some(ref page) = app.page {
                let action = if let Some(ref target_id) = decision.target {
                    page.actions.iter().find(|a| &a.id == target_id).cloned()
                } else {
                    page.actions
                        .iter()
                        .find(|a| a.id.eq_ignore_ascii_case(&decision.operation))
                        .cloned()
                };
                if let Some(act) = action {
                    let _ = browser.act(&act, page, decision.text.as_deref()).await;
                }
            }
            if let Ok(page) = browser.observe(true).await {
                app.elements = actions_to_elements(&page.actions);
                app.screenshot = page.screenshot.clone();
                app.page = Some(page);
            }
            app.browser = Some(browser);
        }
        app.history.push(decision.clone());
        app.decisions.push(decision.clone());
        if decision.operation == "DONE" {
            app.status = "done".to_string();
        } else if decision.operation == "BLOCKED" {
            app.status = "blocked".to_string();
        } else {
            app.status = "ready".to_string();
        }
    }
    (StatusCode::OK, Json(app.to_response()))
}

async fn tick_handler(State(state): State<Arc<RwLock<AppState>>>) -> impl IntoResponse {
    let app = state.read().await;
    (StatusCode::OK, Json(app.to_response()))
}

/// Creates the Axum router with all inspector endpoints and static asset fallback.
pub fn create_router(state: Arc<RwLock<AppState>>) -> Router {
    let static_service =
        tower_http::services::ServeDir::new("static").append_index_html_on_directories(true);

    Router::new()
        .route("/", get(index_handler))
        .route("/api/state", get(get_state_handler))
        .route("/api/start", post(start_handler))
        .route("/api/reset", post(start_handler))
        .route("/api/step", post(step_handler))
        .route("/api/predict", post(predict_handler))
        .route("/api/act", post(act_handler))
        .route("/api/tick", post(tick_handler))
        .fallback_service(static_service)
        .with_state(state)
}

/// Serves the web inspector on the specified port.
pub async fn serve(port: u16) -> anyhow::Result<()> {
    let state = Arc::new(RwLock::new(AppState::default()));
    let app = create_router(state);
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("Starting web inspector server on http://127.0.0.1:{}", port);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

/// CLI runner dispatching to subcommands.
pub async fn run_cli() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    let cli = Cli::parse();
    match cli.command {
        Commands::Serve { port } => {
            println!(
                "Starting Jev Obscura web inspector on http://127.0.0.1:{}...",
                port
            );
            serve(port).await?;
        }
        Commands::Run {
            url,
            goal,
            max_steps,
        } => {
            let cdp_url =
                std::env::var("OBSCURA_CDP_URL").unwrap_or_else(|_| "127.0.0.1:9222".to_string());
            println!("Connecting to Obscura at {}...", cdp_url);
            crate::cdp::ensure_obscura(Some(&cdp_url)).await?;
            let client = crate::cdp::CdpClient::connect(&cdp_url).await?;
            let mut browser = Browser::new(Arc::new(client), None);
            println!("Navigating to {}...", url);
            browser.open(&url).await?;

            let config = AgentConfig {
                max_steps,
                typesafe_base_url: std::env::var("TYPESAFE_BASE_URL").ok(),
                typesafe_api_key: std::env::var("TYPESAFE_API_KEY").ok(),
                typesafe_model: std::env::var("TYPESAFE_MODEL").ok(),
                text_model_base_url: std::env::var("TEXT_MODEL_BASE_URL").ok(),
                text_model_api_key: std::env::var("TEXT_MODEL_API_KEY").ok(),
                text_model: std::env::var("TEXT_MODEL").ok(),
            };

            println!("Running goal: '{}' (max {} steps)...", goal, max_steps);
            let history = Agent::run(&mut browser, &goal, &config).await?;
            println!("Completed in {} steps.", history.len());
            for (i, d) in history.iter().enumerate() {
                println!("Step {}: {} -> {:?}", i + 1, d.operation, d.target);
            }
        }
        Commands::Check => {
            println!("--- Jev Obscura Diagnostic Check ---");
            let cdp_url =
                std::env::var("OBSCURA_CDP_URL").unwrap_or_else(|_| "127.0.0.1:9222".to_string());
            let addr = if cdp_url.contains("://") {
                cdp_url.split("://").nth(1).unwrap_or("127.0.0.1:9222")
            } else {
                &cdp_url
            };
            let host_port = addr.split('/').next().unwrap_or("127.0.0.1:9222");
            match tokio::net::TcpStream::connect(host_port).await {
                Ok(_) => println!("[OK] Obscura CDP reachable at {}", host_port),
                Err(e) => println!("[WARN] Obscura CDP not reachable at {} ({})", host_port, e),
            }

            match std::env::var("TYPESAFE_API_KEY") {
                Ok(val) if !val.is_empty() => println!("[OK] TYPESAFE_API_KEY is configured"),
                _ => println!("[WARN] TYPESAFE_API_KEY is not set (required for online decisions)"),
            }

            let base_url = std::env::var("TYPESAFE_BASE_URL")
                .unwrap_or_else(|_| "https://api.typesafe.ai".to_string());
            println!("[INFO] TYPESAFE_BASE_URL: {}", base_url);

            match std::env::var("TEXT_MODEL_API_KEY").or_else(|_| std::env::var("OPENAI_API_KEY")) {
                Ok(val) if !val.is_empty() => println!("[OK] Text model API key is configured"),
                _ => println!(
                    "[WARN] TEXT_MODEL_API_KEY is not set (required for TYPE_TEXT operations)"
                ),
            }
        }
    }
    Ok(())
}
