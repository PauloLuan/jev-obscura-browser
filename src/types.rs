use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;

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

impl Action {
    pub fn new(
        id: impl Into<String>,
        kind: impl Into<String>,
        label: impl Into<String>,
        role: impl Into<String>,
        value: impl Into<String>,
        node: Option<u64>,
    ) -> Self {
        Self {
            id: id.into(),
            kind: kind.into(),
            label: label.into(),
            role: role.into(),
            value: value.into(),
            node,
            ..Default::default()
        }
    }
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

fn deserialize_scroll<'de, D>(deserializer: D) -> Result<(i64, i64), D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum ScrollHelper {
        Tuple((i64, i64)),
        Obj { y: Option<i64>, height: Option<i64> },
    }

    match ScrollHelper::deserialize(deserializer)? {
        ScrollHelper::Tuple(t) => Ok(t),
        ScrollHelper::Obj { y, height } => Ok((y.unwrap_or(0), height.unwrap_or(0))),
    }
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
