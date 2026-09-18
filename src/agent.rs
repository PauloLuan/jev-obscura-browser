use crate::browser::{Browser, BrowserError};
use crate::model::{self, ModelError};
use crate::types::Decision;

#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("Browser error: {0}")]
    Browser(#[from] BrowserError),
    #[error("Model error: {0}")]
    Model(#[from] ModelError),
    #[error("Max steps exceeded")]
    MaxStepsExceeded,
    #[error("Agent error: {0}")]
    Other(String),
}

#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub max_steps: usize,
    pub typesafe_base_url: Option<String>,
    pub typesafe_api_key: Option<String>,
    pub typesafe_model: Option<String>,
    pub text_model_base_url: Option<String>,
    pub text_model_api_key: Option<String>,
    pub text_model: Option<String>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_steps: 30,
            typesafe_base_url: None,
            typesafe_api_key: None,
            typesafe_model: Some("action".to_string()),
            text_model_base_url: None,
            text_model_api_key: None,
            text_model: Some("deepseek-chat".to_string()),
        }
    }
}

pub struct Agent;

impl Agent {
    /// Executes a single perception-decision-action step in the agent loop.
    pub async fn step(
        browser: &mut Browser,
        goal: &str,
        history: &mut Vec<Decision>,
        config: &AgentConfig,
    ) -> Result<Option<Decision>, AgentError> {
        let http_client = reqwest::Client::new();
        let page = browser.observe(true).await?;

        let typesafe_url = config
            .typesafe_base_url
            .as_deref()
            .unwrap_or("https://api.typesafe.ai");
        let api_key = config.typesafe_api_key.as_deref().unwrap_or("");
        let model = config.typesafe_model.as_deref().unwrap_or("action");

        let history_vals: Vec<serde_json::Value> = history
            .iter()
            .map(|d| serde_json::to_value(d).unwrap_or_default())
            .collect();

        let mut decision = model::choose(
            &http_client,
            typesafe_url,
            api_key,
            model,
            &page,
            goal,
            &history_vals,
        )
        .await?;

        if decision.operation == "DONE" {
            history.push(decision.clone());
            return Ok(Some(decision));
        }

        if decision.operation == "BLOCKED" {
            history.push(decision.clone());
            return Ok(Some(decision));
        }

        if decision.operation == "TYPE_TEXT" && decision.text.is_none() {
            let target_id = decision.target.as_deref().unwrap_or("");
            let target_action = page.actions.iter().find(|a| a.id == target_id);
            let context = serde_json::json!({
                "goal": goal,
                "field": {
                    "id": target_id,
                    "label": target_action.map(|a| a.label.as_str()).unwrap_or(""),
                    "role": target_action.map(|a| a.role.as_str()).unwrap_or("textbox"),
                    "current_value": target_action.and_then(|a| a.current_value.as_deref()).unwrap_or(""),
                },
                "history": &history_vals,
            });

            let text_url = config
                .text_model_base_url
                .as_deref()
                .unwrap_or("https://api.openai.com/v1");
            let text_key = config.text_model_api_key.as_deref().unwrap_or("");
            let text_model = config.text_model.as_deref().unwrap_or("deepseek-chat");

            let generated =
                model::field_text(&http_client, text_url, text_key, text_model, &context).await?;

            decision.text = Some(generated);
        }

        let action = if let Some(ref target_id) = decision.target {
            page.actions.iter().find(|a| &a.id == target_id).cloned()
        } else {
            page.actions
                .iter()
                .find(|a| a.id.eq_ignore_ascii_case(&decision.operation))
                .cloned()
        }
        .ok_or_else(|| {
            AgentError::Other(format!(
                "Action for decision {:?} not found in page",
                decision
            ))
        })?;

        browser
            .act(&action, &page, decision.text.as_deref())
            .await?;
        history.push(decision.clone());
        Ok(Some(decision))
    }

    /// Runs the autonomous agent loop up to config.max_steps.
    pub async fn run(
        browser: &mut Browser,
        goal: &str,
        config: &AgentConfig,
    ) -> Result<Vec<Decision>, AgentError> {
        let mut history = Vec::new();
        for _ in 0..config.max_steps {
            match Self::step(browser, goal, &mut history, config).await? {
                Some(decision)
                    if decision.operation == "DONE" || decision.operation == "BLOCKED" =>
                {
                    return Ok(history);
                }
                Some(_) => continue,
                None => break,
            }
        }

        if history.last().map(|d| d.operation.as_str()) != Some("DONE") {
            return Err(AgentError::MaxStepsExceeded);
        }

        Ok(history)
    }
}
