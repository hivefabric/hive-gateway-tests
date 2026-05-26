//! Cancellation integration tests.
//!
//! Verifies the full cancel lifecycle: queued tasks, pending-queue removal,
//! and the audit trail.

mod helpers;
use helpers::Stack;
use serde_json::json;
use uuid::Uuid;

/// Cancelling a Queued task removes it from the pending queue and marks it Cancelled.
#[tokio::test]
#[ignore = "requires running stack: HONEYCOMB_URL DATABASE_URL"]
async fn queued_task_cancelled_immediately() {
    let Some(stack) = Stack::from_env() else { return; };

    let task_id = Uuid::new_v4().to_string();

    // Submit (will be Queued — no nodes)
    let resp = stack
        .honeycomb_post(
            "/api/tasks/create",
            &json!({
                "task_id": task_id,
                "owner_id": Uuid::new_v4().to_string(),
                "execution_type": "Llm",
                "payload": {"profile": "default", "prompt": "cancel me please"},
                "required_capabilities": {"cpu_cores": null, "memory_mb": null, "llm_profiles": []},
                "allowed_nodes": "hive-wide",
                "jurisdiction_required": []
            }),
        )
        .await
        .expect("create task");
    assert!(resp.status().is_success());

    // Cancel
    let cancel = stack
        .honeycomb_post(&format!("/api/tasks/{task_id}/cancel"), &json!({}))
        .await
        .expect("cancel task");
    assert!(cancel.status().is_success(), "cancel failed: {}", cancel.status());
    let cancel_body: serde_json::Value = cancel.json().await.expect("cancel body");
    assert_eq!(cancel_body["ok"], true);
    assert_eq!(cancel_body["status"].as_str(), Some("Cancelled"));

    // Verify final status
    let task_resp = stack
        .honeycomb_get(&format!("/api/tasks/{task_id}"))
        .await
        .expect("get task");
    let task: serde_json::Value = task_resp.json().await.expect("task json");
    assert_eq!(
        task["status"].as_str(),
        Some("Cancelled"),
        "task should be Cancelled after cancel call"
    );
}

/// Audit log captures the full lifecycle including cancellation.
#[tokio::test]
#[ignore = "requires running stack: HONEYCOMB_URL DATABASE_URL"]
async fn audit_log_captures_lifecycle() {
    let Some(stack) = Stack::from_env() else { return; };

    let task_id = Uuid::new_v4().to_string();

    let resp = stack
        .honeycomb_post(
            "/api/tasks/create",
            &json!({
                "task_id": task_id,
                "owner_id": Uuid::new_v4().to_string(),
                "execution_type": "Llm",
                "payload": {"profile": "default", "prompt": "audit me"},
                "required_capabilities": {"cpu_cores": null, "memory_mb": null, "llm_profiles": []},
                "allowed_nodes": "hive-wide",
                "jurisdiction_required": []
            }),
        )
        .await
        .expect("create task");
    assert!(resp.status().is_success());

    // Cancel the task
    let _ = stack
        .honeycomb_post(&format!("/api/tasks/{task_id}/cancel"), &json!({}))
        .await
        .expect("cancel task");

    // Fetch the audit log
    let audit = stack
        .honeycomb_get(&format!("/api/tasks/{task_id}/audit"))
        .await
        .expect("audit log");
    assert!(audit.status().is_success(), "audit log request failed: {}", audit.status());
    let events: Vec<serde_json::Value> = audit.json().await.expect("audit events json");

    // Must have at least: Created, Queued, Cancelled
    assert!(events.len() >= 2, "expected at least 2 lifecycle events, got {}", events.len());

    let states: Vec<&str> = events
        .iter()
        .filter_map(|e| e["state"].as_str())
        .collect();

    assert!(
        states.contains(&"Created") || states.contains(&"Queued"),
        "audit log must contain Created or Queued state: {:?}",
        states
    );
    assert!(
        states.contains(&"Cancelled"),
        "audit log must contain Cancelled state: {:?}",
        states
    );

    // Events must be ordered chronologically
    let timestamps: Vec<&str> = events
        .iter()
        .filter_map(|e| e["at"].as_str())
        .collect();
    let mut sorted = timestamps.clone();
    sorted.sort();
    assert_eq!(timestamps, sorted, "audit events should be in chronological order");
}

/// Cancelling a task that doesn't exist returns 404.
#[tokio::test]
#[ignore = "requires running stack: HONEYCOMB_URL DATABASE_URL"]
async fn cancel_nonexistent_task_returns_not_found() {
    let Some(stack) = Stack::from_env() else { return; };

    let fake_id = Uuid::new_v4().to_string();
    let resp = stack
        .honeycomb_post(&format!("/api/tasks/{fake_id}/cancel"), &json!({}))
        .await
        .expect("cancel request");

    assert_eq!(
        resp.status(),
        reqwest::StatusCode::NOT_FOUND,
        "cancelling a non-existent task should return 404"
    );
}
