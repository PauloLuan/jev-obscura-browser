use std::collections::{HashMap, HashSet};

use crate::questions::build_questions;
use crate::types::{Action, Choice, Decision, PageState};

#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("Invalid choice: {0}")]
    InvalidChoice(String),
    #[error("API error: {0}")]
    ApiError(String),
    #[error("Network error: {0}")]
    NetworkError(#[from] reqwest::Error),
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    #[error("Missing credential: {0}")]
    MissingCredential(String),
    #[error("Model error: {0}")]
    Other(String),
}

pub type ActionSpace = (
    HashMap<String, Action>,
    HashMap<String, Vec<String>>,
    Vec<String>,
);

pub fn action_space(actions: &[Action]) -> ActionSpace {
    let mut elements = HashMap::new();
    let mut targets: HashMap<String, Vec<String>> = HashMap::new();
    let mut controls: Vec<String> = Vec::new();

    for action in actions {
        match action.kind.as_str() {
            "fill" | "type_text" => {
                elements.insert(action.id.clone(), action.clone());
                targets
                    .entry("TYPE_TEXT".to_string())
                    .or_default()
                    .push(action.id.clone());
            }
            "click" => {
                elements.insert(action.id.clone(), action.clone());
                targets
                    .entry("CLICK".to_string())
                    .or_default()
                    .push(action.id.clone());
            }
            "select" => {
                elements.insert(action.id.clone(), action.clone());
                targets
                    .entry("SELECT".to_string())
                    .or_default()
                    .push(action.id.clone());
            }
            _ => {
                let ctrl = action.id.to_uppercase();
                if !controls.contains(&ctrl) {
                    controls.push(ctrl);
                }
            }
        }
    }

    if !controls.contains(&"WAIT".to_string()) {
        controls.push("WAIT".to_string());
    }
    if !controls.contains(&"DONE".to_string()) {
        controls.push("DONE".to_string());
    }

    (elements, targets, controls)
}

pub fn validate_choice(choice: &Choice, allowed: &HashSet<String>) -> Result<(), ModelError> {
    if !allowed.contains(&choice.choice) {
        return Err(ModelError::InvalidChoice(format!(
            "Choice '{}' not in allowed set",
            choice.choice
        )));
    }

    if choice.probabilities.len() != allowed.len() {
        return Err(ModelError::InvalidChoice(format!(
            "Probability keys count {} does not match allowed count {}",
            choice.probabilities.len(),
            allowed.len()
        )));
    }

    for key in choice.probabilities.keys() {
        if !allowed.contains(key) {
            return Err(ModelError::InvalidChoice(format!(
                "Probability key '{}' not in allowed set",
                key
            )));
        }
    }

    if !choice.confidence.is_finite() || !(0.0..=1.0).contains(&choice.confidence) {
        return Err(ModelError::InvalidChoice(format!(
            "Invalid confidence: {}",
            choice.confidence
        )));
    }

    let mut sum = 0.0;
    let mut max_prob = f64::NEG_INFINITY;
    for &prob in choice.probabilities.values() {
        if !prob.is_finite() || !(0.0..=1.0).contains(&prob) {
            return Err(ModelError::InvalidChoice(format!(
                "Invalid probability: {}",
                prob
            )));
        }
        sum += prob;
        if prob > max_prob {
            max_prob = prob;
        }
    }

    if (sum - 1.0).abs() >= 0.02 {
        return Err(ModelError::InvalidChoice(format!(
            "Probabilities sum to {}, expected ~1.0",
            sum
        )));
    }

    let chosen_prob = choice
        .probabilities
        .get(&choice.choice)
        .copied()
        .unwrap_or(0.0);
    if chosen_prob < max_prob - 1e-6 {
        return Err(ModelError::InvalidChoice(format!(
            "Chosen probability {} is not maximal (max was {})",
            chosen_prob, max_prob
        )));
    }

    Ok(())
}

pub async fn choose(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    model: &str,
    page: &PageState,
    goal: &str,
    history: &[serde_json::Value],
) -> Result<Decision, ModelError> {
    let (elements, targets, controls) = action_space(&page.actions);
    let questions = build_questions(goal, &targets, &elements, &controls);

    let url = format!("{}/v1/systemone", base_url.trim_end_matches('/'));
    let recent_actions: Vec<serde_json::Value> =
        history.iter().rev().take(10).rev().cloned().collect();

    let body = serde_json::json!({
        "model": model,
        "state": {
            "page": {
                "url": page.url,
                "title": page.title,
                "text": page.text
            },
            "elements": elements,
            "recent_actions": recent_actions,
        },
        "questions": questions
    });

    let mut req = client.post(&url).json(&body);
    if !api_key.is_empty() {
        req = req.header("Authorization", format!("Bearer {}", api_key));
    }

    let res = req.send().await?;
    if !res.status().is_success() {
        return Err(ModelError::ApiError(format!(
            "Model provider returned HTTP {}",
            res.status()
        )));
    }

    let result_json: serde_json::Value = res.json().await?;
    let answers = result_json
        .get("answers")
        .ok_or_else(|| ModelError::ApiError("Response missing 'answers' field".to_string()))?;

    let op_answer_val = answers
        .get("operation")
        .ok_or_else(|| ModelError::ApiError("Answers missing 'operation'".to_string()))?;
    let op_choice: Choice = serde_json::from_value(op_answer_val.clone())?;

    let mut allowed_ops: HashSet<String> = targets.keys().cloned().collect();
    for c in &controls {
        allowed_ops.insert(c.clone());
    }
    validate_choice(&op_choice, &allowed_ops)?;

    let operation = op_choice.choice;
    let mut target = None;

    if targets.contains_key(&operation) {
        let head_key = format!("{}_target", operation.to_lowercase());
        let target_answer_val = answers
            .get(&head_key)
            .ok_or_else(|| ModelError::ApiError(format!("Answers missing '{}'", head_key)))?;
        let target_choice: Choice = serde_json::from_value(target_answer_val.clone())?;
        let allowed_candidates: HashSet<String> = targets[&operation].iter().cloned().collect();
        validate_choice(&target_choice, &allowed_candidates)?;
        target = Some(target_choice.choice);
    }

    Ok(Decision {
        operation,
        target,
        text: None,
    })
}

pub async fn field_text(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    model: &str,
    context: &serde_json::Value,
) -> Result<String, ModelError> {
    if api_key.trim().is_empty() {
        return Err(ModelError::MissingCredential(
            "TYPE_TEXT needs TEXT_MODEL_API_KEY; no text is hardcoded or guessed by the executor."
                .to_string(),
        ));
    }

    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let body = serde_json::json!({
        "model": model,
        "max_tokens": 1024,
        "response_format": {"type": "json_object"},
        "messages": [
            {"role": "system", "content": crate::questions::TEXT_VALUE},
            {"role": "user", "content": context.to_string()}
        ]
    });

    let res = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&body)
        .send()
        .await?;

    if !res.status().is_success() {
        return Err(ModelError::ApiError(format!(
            "Text helper returned HTTP {}",
            res.status()
        )));
    }

    let result_json: serde_json::Value = res.json().await?;
    let content = result_json
        .get("choices")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|msg| msg.get("content"))
        .and_then(|cnt| cnt.as_str())
        .ok_or_else(|| ModelError::ApiError("Missing content in text response".to_string()))?;

    let parsed: serde_json::Value = serde_json::from_str(content).map_err(|_| {
        ModelError::ApiError(
            "Text helper returned no valid field value; nothing typed.".to_string(),
        )
    })?;

    let text_val = parsed.get("text").ok_or_else(|| {
        ModelError::ApiError(
            "Text helper returned no valid field value; nothing typed.".to_string(),
        )
    })?;

    if let Some(text_str) = text_val.as_str() {
        if text_str.trim().is_empty() || text_str.len() > 2000 {
            return Err(ModelError::ApiError(
                "Text helper returned no valid field value; nothing typed.".to_string(),
            ));
        }
        Ok(text_str.to_string())
    } else {
        Err(ModelError::ApiError(
            "Text helper returned no valid field value; nothing typed.".to_string(),
        ))
    }
}
