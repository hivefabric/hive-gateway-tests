//! Smoke tests — basic end-to-end flow.
//!
//! Requires: HONEYCOMB_URL and GATEWAY_URL env vars pointing to a running stack.

mod helpers;
use helpers::Stack;
use serde_json::json;
use uuid::Uuid;

/// Healthz endpoint returns ok.
#[tokio::test]
#[ignore = "requires running stack: HONEYCOMB_URL GATEWAY_URL"]
async fn honeycomb_healthz() {
    let Some(stack) = Stack::from_env() else { return; };
    let resp = stack.honeycomb_get("/healthz").await.expect("request");
    assert!(resp.status().is_success(), "healthz failed: {}", resp.status());
    let body: serde_json::Value = resp.json().await.expect("json");
    assert_eq!(body["ok"], true);
}

/// Self-service signup flow: POST /v1/signup returns a tenant_id and api_key.
#[tokio::test]
#[ignore = "requires running stack: GATEWAY_URL"]
async fn gateway_self_service_signup() {
    let Some(stack) = Stack::from_env() else { return; };
    let (tenant_id, api_key) = stack.signup("test-smoke-tenant").await;
    assert!(!tenant_id.is_empty(), "tenant_id should be non-empty");
    assert!(api_key.starts_with("hf_"), "api_key should start with hf_");
}

/// Signup + register LLM provider + list providers round-trip.
#[tokio::test]
#[ignore = "requires running stack: GATEWAY_URL"]
async fn gateway_llm_provider_crud() {
    let Some(stack) = Stack::from_env() else { return; };
    let (_tenant_id, api_key) = stack.signup("test-provider-tenant").await;

    // Register a provider
    let resp = stack
        .gateway_post(
            "/v1/me/llm-providers",
            &api_key,
            &json!({
                "name": "test-anthropic",
                "provider": "anthropic",
                "model": "claude-haiku-4-5",
                "api_key": "sk-ant-test-key-placeholder",
                "is_default": true
            }),
        )
        .await
        .expect("register provider");
    assert!(resp.status().is_success(), "register provider failed: {}", resp.status());
    let provider: serde_json::Value = resp.json().await.expect("provider json");
    let provider_id = provider["id"].as_str().expect("provider id");
    assert!(!provider_id.is_empty());
    assert_eq!(provider["is_default"], true);

    // List providers
    let resp = stack
        .client
        .get(format!("{}/v1/me/llm-providers", stack.gateway_url))
        .header("authorization", format!("bearer {api_key}"))
        .send()
        .await
        .expect("list providers");
    assert!(resp.status().is_success());
    let providers: serde_json::Value = resp.json().await.expect("providers json");
    assert!(providers.as_array().map(|a| a.len()).unwrap_or(0) >= 1);
}

/// Node registration → task submit → task in task list.
#[tokio::test]
#[ignore = "requires running stack: HONEYCOMB_URL GATEWAY_URL DATABASE_URL"]
async fn end_to_end_task_queued_when_no_nodes() {
    let Some(stack) = Stack::from_env() else { return; };

    // Submit a task when no nodes are registered — it should be Queued.
    let task_id = Uuid::new_v4().to_string();
    let resp = stack
        .honeycomb_post(
            "/api/tasks/create",
            &json!({
                "task_id": task_id,
                "owner_id": Uuid::new_v4().to_string(),
                "execution_type": "Llm",
                "payload": {"profile": "default", "prompt": "2 + 2 ="},
                "required_capabilities": {"cpu_cores": null, "memory_mb": null, "llm_profiles": []},
                "allowed_nodes": "hive-wide",
                "jurisdiction_required": []
            }),
        )
        .await
        .expect("create task");
    assert!(resp.status().is_success(), "create task failed: {}", resp.status());
    let body: serde_json::Value = resp.json().await.expect("task response json");
    let returned_task_id = body["task_id"].as_str().expect("task_id in response");
    assert_eq!(returned_task_id, task_id);

    // Task should be in the task list
    let list_resp = stack
        .honeycomb_get("/api/tasks")
        .await
        .expect("list tasks");
    assert!(list_resp.status().is_success());
    let tasks: Vec<serde_json::Value> = list_resp.json().await.expect("tasks json");
    let found = tasks.iter().any(|t| t["task_id"].as_str() == Some(&task_id));
    assert!(found, "submitted task should appear in the task list");
}

/// Task can be cancelled via POST /api/tasks/{id}/cancel.
#[tokio::test]
#[ignore = "requires running stack: HONEYCOMB_URL DATABASE_URL"]
async fn task_cancel_via_api() {
    let Some(stack) = Stack::from_env() else { return; };

    // Submit a task (it will be Queued since there are no nodes)
    let task_id = Uuid::new_v4().to_string();
    let resp = stack
        .honeycomb_post(
            "/api/tasks/create",
            &json!({
                "task_id": task_id,
                "owner_id": Uuid::new_v4().to_string(),
                "execution_type": "Llm",
                "payload": {"profile": "default", "prompt": "cancel me"},
                "required_capabilities": {"cpu_cores": null, "memory_mb": null, "llm_profiles": []},
                "allowed_nodes": "hive-wide",
                "jurisdiction_required": []
            }),
        )
        .await
        .expect("create task");
    assert!(resp.status().is_success());

    // Cancel it
    let cancel_resp = stack
        .honeycomb_post(&format!("/api/tasks/{task_id}/cancel"), &json!({}))
        .await
        .expect("cancel task");
    assert!(cancel_resp.status().is_success(), "cancel failed: {}", cancel_resp.status());

    // Verify it's cancelled
    let task_resp = stack
        .honeycomb_get(&format!("/api/tasks/{task_id}"))
        .await
        .expect("get task");
    let task: serde_json::Value = task_resp.json().await.expect("task json");
    assert_eq!(task["status"].as_str(), Some("Cancelled"));

    // Audit log should show the cancellation
    let audit_resp = stack
        .honeycomb_get(&format!("/api/tasks/{task_id}/audit"))
        .await
        .expect("audit log");
    assert!(audit_resp.status().is_success());
    let events: Vec<serde_json::Value> = audit_resp.json().await.expect("audit events");
    assert!(!events.is_empty(), "audit log should have at least one event");
    let has_cancelled = events.iter().any(|e| e["state"].as_str() == Some("Cancelled"));
    assert!(has_cancelled, "audit log should contain a Cancelled state event");
}
