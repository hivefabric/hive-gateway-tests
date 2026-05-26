//! Queen decomposition integration tests.
//!
//! Verifies: queen task spawns sub-tasks, sub-tasks appear under
//! GET /api/tasks/{id}/children, parent_task_id is set on sub-tasks.
//!
//! Requires: HONEYCOMB_URL + DATABASE_URL env vars, and at least one
//! queen-capable comb registered.

mod helpers;
use helpers::Stack;
use serde_json::json;
use uuid::Uuid;

/// A queen task creates sub-tasks that appear via the children endpoint.
/// NOTE: This test requires a queen comb registered with
/// oasf://demo/queen/decompose/v1 capability and an appropriate LLM configured.
/// Without a running queen comb the task will be Queued; children will be empty.
#[tokio::test]
#[ignore = "requires running stack with queen comb: HONEYCOMB_URL DATABASE_URL"]
async fn queen_task_children_are_linked() {
    let Some(stack) = Stack::from_env() else { return; };

    let task_id = Uuid::new_v4().to_string();
    let resp = stack
        .honeycomb_post(
            "/api/tasks/create",
            &json!({
                "task_id": task_id,
                "owner_id": Uuid::new_v4().to_string(),
                "execution_type": "llm",
                "payload": {"prompt": "What is 3 + 4?"},
                "required_capabilities": {"cpu_cores": null, "memory_mb": null, "llm_profiles": []},
                "allowed_nodes": "hive-wide",
                "capability_urn": {
                    "namespace": "demo",
                    "domain": "queen",
                    "operation": "decompose",
                    "version": 1
                },
                "jurisdiction_required": []
            }),
        )
        .await
        .expect("create queen task");
    assert!(resp.status().is_success());

    // Poll until terminal (give queen time to decompose)
    let final_task = stack.poll_task(&task_id, 60).await;

    // Whether the task succeeded or just queued, the children endpoint must work
    let children_resp = stack
        .honeycomb_get(&format!("/api/tasks/{task_id}/children"))
        .await
        .expect("children request");
    assert!(children_resp.status().is_success(), "children endpoint should return 200");

    let children: Vec<serde_json::Value> = children_resp.json().await.expect("children json");

    // If the queen ran, each sub-task should have parent_task_id set
    for child in &children {
        let parent = child.get("parent_task_id").and_then(serde_json::Value::as_str);
        assert_eq!(
            parent,
            Some(task_id.as_str()),
            "child task should have parent_task_id = queen's task_id"
        );
    }

    // If the task succeeded, there should be at least one sub-task
    if final_task["status"].as_str() == Some("Succeeded") {
        assert!(
            !children.is_empty(),
            "a completed queen task should have spawned at least one sub-task"
        );
    }
}

/// The children endpoint returns an empty array (not 404) for non-queen tasks.
#[tokio::test]
#[ignore = "requires running stack: HONEYCOMB_URL DATABASE_URL"]
async fn non_queen_task_has_empty_children() {
    let Some(stack) = Stack::from_env() else { return; };

    let task_id = Uuid::new_v4().to_string();
    let resp = stack
        .honeycomb_post(
            "/api/tasks/create",
            &json!({
                "task_id": task_id,
                "owner_id": Uuid::new_v4().to_string(),
                "execution_type": "Llm",
                "payload": {"profile": "default", "prompt": "hello"},
                "required_capabilities": {"cpu_cores": null, "memory_mb": null, "llm_profiles": []},
                "allowed_nodes": "hive-wide",
                "jurisdiction_required": []
            }),
        )
        .await
        .expect("create task");
    assert!(resp.status().is_success());

    let children_resp = stack
        .honeycomb_get(&format!("/api/tasks/{task_id}/children"))
        .await
        .expect("children request");
    assert!(children_resp.status().is_success());

    let children: Vec<serde_json::Value> = children_resp.json().await.expect("children json");
    assert!(children.is_empty(), "non-queen task should have no children");

    // Cleanup
    let _ = stack.honeycomb_post(&format!("/api/tasks/{task_id}/cancel"), &json!({})).await;
}
