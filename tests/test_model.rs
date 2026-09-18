use std::collections::HashSet;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use jev_obscura_browser::model::{action_space, choose, field_text, validate_choice, ModelError};
use jev_obscura_browser::questions::{build_questions, ALLOWED_OPERATIONS};
use jev_obscura_browser::types::{Action, Choice, PageState};

fn sample_page() -> PageState {
    PageState {
        url: "https://example.test/search".into(),
        title: "Test Page".into(),
        text: "Search for books\nSubmit form".into(),
        scroll: (0, 1000),
        actions: vec![
            Action {
                id: "e1".into(),
                kind: "fill".into(),
                label: "Search input".into(),
                role: "textbox".into(),
                value: "".into(),
                node: Some(10),
                ..Default::default()
            },
            Action {
                id: "e2".into(),
                kind: "click".into(),
                label: "Submit button".into(),
                role: "button".into(),
                value: "".into(),
                node: Some(20),
                ..Default::default()
            },
            Action {
                id: "e3".into(),
                kind: "select".into(),
                label: "Category dropdown".into(),
                role: "combobox".into(),
                value: "books".into(),
                node: Some(30),
                ..Default::default()
            },
            Action {
                id: "wait".into(),
                kind: "wait".into(),
                label: "Wait for page load".into(),
                ..Default::default()
            },
            Action {
                id: "scroll_down".into(),
                kind: "scroll".into(),
                label: "Scroll down".into(),
                delta: Some(500),
                ..Default::default()
            },
        ],
        screenshot: None,
    }
}

#[test]
fn test_invalid_choice_rejected() {
    let allowed: HashSet<String> = ["a".into(), "b".into()].into_iter().collect();

    // Valid choice
    let valid = Choice {
        choice: "a".into(),
        confidence: 0.95,
        probabilities: [("a".into(), 0.95), ("b".into(), 0.05)]
            .into_iter()
            .collect(),
    };
    assert!(validate_choice(&valid, &allowed).is_ok());

    // Choice not in allowed set
    let unseen = Choice {
        choice: "unseen".into(),
        confidence: 1.0,
        probabilities: [("a".into(), 0.5), ("b".into(), 0.5)].into_iter().collect(),
    };
    assert!(validate_choice(&unseen, &allowed).is_err());

    // Missing key in probabilities
    let missing_key = Choice {
        choice: "a".into(),
        confidence: 1.0,
        probabilities: [("a".into(), 1.0)].into_iter().collect(),
    };
    assert!(validate_choice(&missing_key, &allowed).is_err());

    // Chosen element is not maximum probability
    let non_max = Choice {
        choice: "b".into(),
        confidence: 0.2,
        probabilities: [("a".into(), 0.8), ("b".into(), 0.2)].into_iter().collect(),
    };
    assert!(validate_choice(&non_max, &allowed).is_err());

    // Sum of probabilities far from 1.0
    let bad_sum = Choice {
        choice: "a".into(),
        confidence: 0.6,
        probabilities: [("a".into(), 0.6), ("b".into(), 0.1)].into_iter().collect(),
    };
    assert!(validate_choice(&bad_sum, &allowed).is_err());

    // Negative probability
    let neg_prob = Choice {
        choice: "a".into(),
        confidence: 1.0,
        probabilities: [("a".into(), 1.2), ("b".into(), -0.2)]
            .into_iter()
            .collect(),
    };
    assert!(validate_choice(&neg_prob, &allowed).is_err());

    // NaN probability
    let nan_prob = Choice {
        choice: "a".into(),
        confidence: 1.0,
        probabilities: [("a".into(), f64::NAN), ("b".into(), 0.0)]
            .into_iter()
            .collect(),
    };
    assert!(validate_choice(&nan_prob, &allowed).is_err());
}

#[test]
fn test_action_space_partitioning() {
    let page = sample_page();
    let (elements, targets, controls) = action_space(&page.actions);

    // Elements lookup contains the interactive actions
    assert_eq!(elements.len(), 3);
    assert!(elements.contains_key("e1"));
    assert!(elements.contains_key("e2"));
    assert!(elements.contains_key("e3"));

    // Targets contains interactive operations
    assert!(targets.contains_key("TYPE_TEXT"));
    assert_eq!(targets["TYPE_TEXT"], vec!["e1".to_string()]);

    assert!(targets.contains_key("CLICK"));
    assert_eq!(targets["CLICK"], vec!["e2".to_string()]);

    assert!(targets.contains_key("SELECT"));
    assert_eq!(targets["SELECT"], vec!["e3".to_string()]);

    // Controls contains control actions including WAIT, DONE, and SCROLL_DOWN
    assert!(controls.contains(&"WAIT".to_string()));
    assert!(controls.contains(&"DONE".to_string()));
    assert!(controls.contains(&"SCROLL_DOWN".to_string()));
}

#[test]
fn test_typesafe_request_building() {
    let page = sample_page();
    let (elements, targets, controls) = action_space(&page.actions);
    let questions = build_questions("Find a Rust book", &targets, &elements, &controls);

    // Operation question must be present
    assert!(questions.get("operation").is_some());
    let op_q = &questions["operation"];
    assert_eq!(op_q["type"], "choice");

    // Check operation criteria contains allowed operations
    let criteria = op_q["criteria"].as_object().unwrap();
    assert!(criteria.contains_key("TYPE_TEXT"));
    assert!(criteria.contains_key("CLICK"));
    assert!(criteria.contains_key("SELECT"));
    assert!(criteria.contains_key("WAIT"));
    assert!(criteria.contains_key("DONE"));

    for op in criteria.keys() {
        assert!(ALLOWED_OPERATIONS.contains(&op.as_str()));
    }

    // Speculative target head questions must be present
    assert!(questions.get("type_text_target").is_some());
    assert!(questions.get("click_target").is_some());
    assert!(questions.get("select_target").is_some());

    let type_text_candidates = questions["type_text_target"]["criteria"]
        .as_object()
        .unwrap();
    assert!(type_text_candidates.contains_key("e1"));
}

#[tokio::test]
async fn test_choose_mock_prediction() {
    let mock_server = MockServer::start().await;
    let page = sample_page();

    let response_body = serde_json::json!({
        "model": "jev-latest",
        "answers": {
            "operation": {
                "choice": "TYPE_TEXT",
                "confidence": 0.98,
                "probabilities": {
                    "TYPE_TEXT": 0.98,
                    "CLICK": 0.01,
                    "SELECT": 0.0,
                    "WAIT": 0.01,
                    "DONE": 0.0,
                    "SCROLL_DOWN": 0.0
                }
            },
            "type_text_target": {
                "choice": "e1",
                "confidence": 0.99,
                "probabilities": {
                    "e1": 1.0
                }
            },
            "click_target": {
                "choice": "e2",
                "confidence": 0.90,
                "probabilities": {
                    "e2": 1.0
                }
            }
        }
    });

    Mock::given(method("POST"))
        .and(path("/v1/systemone"))
        .and(header("Authorization", "Bearer test-api-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = reqwest::Client::new();
    let decision = choose(
        &client,
        &mock_server.uri(),
        "test-api-key",
        "jev-latest",
        &page,
        "Search for a book",
        &[],
    )
    .await
    .expect("choose should succeed");

    assert_eq!(decision.operation, "TYPE_TEXT");
    assert_eq!(decision.target, Some("e1".to_string()));
    assert_eq!(decision.text, None);
}

#[tokio::test]
async fn test_choose_unselected_target_head_does_not_fail() {
    let mock_server = MockServer::start().await;
    let page = sample_page();

    // click_target has an invalid/invented choice, but operation is TYPE_TEXT
    let response_body = serde_json::json!({
        "model": "jev-latest",
        "answers": {
            "operation": {
                "choice": "TYPE_TEXT",
                "confidence": 1.0,
                "probabilities": {
                    "TYPE_TEXT": 1.0,
                    "CLICK": 0.0,
                    "SELECT": 0.0,
                    "WAIT": 0.0,
                    "DONE": 0.0,
                    "SCROLL_DOWN": 0.0
                }
            },
            "type_text_target": {
                "choice": "e1",
                "confidence": 1.0,
                "probabilities": {
                    "e1": 1.0
                }
            },
            "click_target": {
                "choice": "invented_element_999",
                "confidence": 1.0,
                "probabilities": {}
            }
        }
    });

    Mock::given(method("POST"))
        .and(path("/v1/systemone"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
        .mount(&mock_server)
        .await;

    let client = reqwest::Client::new();
    let decision = choose(
        &client,
        &mock_server.uri(),
        "test-api-key",
        "jev-latest",
        &page,
        "Search for a book",
        &[],
    )
    .await
    .expect("choose should succeed despite invalid unselected head");

    assert_eq!(decision.operation, "TYPE_TEXT");
    assert_eq!(decision.target, Some("e1".to_string()));
}

#[tokio::test]
async fn test_choose_control_action() {
    let mock_server = MockServer::start().await;
    let page = sample_page();

    let response_body = serde_json::json!({
        "model": "jev-latest",
        "answers": {
            "operation": {
                "choice": "WAIT",
                "confidence": 0.99,
                "probabilities": {
                    "TYPE_TEXT": 0.0,
                    "CLICK": 0.0,
                    "SELECT": 0.0,
                    "WAIT": 1.0,
                    "DONE": 0.0,
                    "SCROLL_DOWN": 0.0
                }
            }
        }
    });

    Mock::given(method("POST"))
        .and(path("/v1/systemone"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
        .mount(&mock_server)
        .await;

    let client = reqwest::Client::new();
    let decision = choose(
        &client,
        &mock_server.uri(),
        "test-api-key",
        "jev-latest",
        &page,
        "Wait for results",
        &[],
    )
    .await
    .expect("choose should succeed for control action");

    assert_eq!(decision.operation, "WAIT");
    assert_eq!(decision.target, None);
    assert_eq!(decision.text, None);
}

#[tokio::test]
async fn test_field_text_mock_prediction() {
    let mock_server = MockServer::start().await;

    let response_body = serde_json::json!({
        "choices": [
            {
                "message": {
                    "content": "{\"text\": \"The Rust Programming Language\"}"
                }
            }
        ]
    });

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(header("Authorization", "Bearer text-api-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
        .expect(1)
        .mount(&mock_server)
        .await;

    let client = reqwest::Client::new();
    let context = serde_json::json!({
        "goal": "Search for Rust books",
        "field": {"label": "Search input", "role": "textbox"}
    });

    let text = field_text(
        &client,
        &mock_server.uri(),
        "text-api-key",
        "deepseek-chat",
        &context,
    )
    .await
    .expect("field_text should succeed");

    assert_eq!(text, "The Rust Programming Language");
}

#[tokio::test]
async fn test_field_text_invalid_json_rejected() {
    let mock_server = MockServer::start().await;

    let response_body = serde_json::json!({
        "choices": [
            {
                "message": {
                    "content": "Thinking: book search"
                }
            }
        ]
    });

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
        .mount(&mock_server)
        .await;

    let client = reqwest::Client::new();
    let context = serde_json::json!({"goal": "Search"});

    let result = field_text(
        &client,
        &mock_server.uri(),
        "text-api-key",
        "deepseek-chat",
        &context,
    )
    .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_field_text_missing_api_key() {
    let client = reqwest::Client::new();
    let context = serde_json::json!({"goal": "Search"});

    let result = field_text(&client, "http://localhost", "", "deepseek-chat", &context).await;

    match result {
        Err(ModelError::MissingCredential(msg)) => {
            assert!(msg.contains("TEXT_MODEL_API_KEY"));
        }
        _ => panic!("Expected MissingCredential error"),
    }
}
