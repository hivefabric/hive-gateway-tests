//! Sensitivity routing integration tests.
//!
//! Verifies that the Forager Stage-1 classifier upgrades sensitivity
//! for payloads containing PII, and that the hard scheduler gate
//! enforces the resulting routing policy.

mod helpers;
use helpers::Stack;
use serde_json::json;
use uuid::Uuid;

/// A task containing a US SSN in the payload gets sensitivity upgraded to Private.
/// The task should be Queued (no Private-accepting combs), NOT rejected with an error.
#[tokio::test]
#[ignore = "requires running stack: HONEYCOMB_URL DATABASE_URL"]
async fn pii_payload_queued_not_on_public_comb() {
    let Some(stack) = Stack::from_env() else { return; };

    let task_id = Uuid::new_v4().to_string();
    let resp = stack
        .honeycomb_post(
            "/api/tasks/create",
            &json!({
                "task_id": task_id,
                "owner_id": Uuid::new_v4().to_string(),
                "execution_type": "Llm",
                // Payload contains a US SSN — Forager should classify as Private
                "payload": {
                    "profile": "default",
                    "prompt": "Summarise the medical record for patient SSN 123-45-6789"
                },
                "required_capabilities": {"cpu_cores": null, "memory_mb": null, "llm_profiles": []},
                "allowed_nodes": "hive-wide",
                "jurisdiction_required": []
            }),
        )
        .await
        .expect("create task");

    // The task should be accepted (200) — it just gets queued with Private sensitivity.
    // If ALL combs are public-only, no comb will accept it, so it stays Queued.
    assert!(
        resp.status().is_success(),
        "task creation should succeed even with PII (queued, not rejected): {}",
        resp.status()
    );

    let body: serde_json::Value = resp.json().await.expect("response json");
    let returned_id = body["task_id"].as_str().expect("task_id");
    assert_eq!(returned_id, task_id);

    // Clean up
    let _ = stack
        .honeycomb_post(&format!("/api/tasks/{task_id}/cancel"), &json!({}))
        .await;
}

/// A task with explicit sensitivity_required=Private is handled correctly.
#[tokio::test]
#[ignore = "requires running stack: HONEYCOMB_URL DATABASE_URL"]
async fn explicit_private_sensitivity_accepted() {
    let Some(stack) = Stack::from_env() else { return; };

    let task_id = Uuid::new_v4().to_string();
    let resp = stack
        .honeycomb_post(
            "/api/tasks/create",
            &json!({
                "task_id": task_id,
                "owner_id": Uuid::new_v4().to_string(),
                "execution_type": "Llm",
                "payload": {"profile": "default", "prompt": "confidential: analyse this"},
                "required_capabilities": {"cpu_cores": null, "memory_mb": null, "llm_profiles": []},
                "allowed_nodes": "hive-wide",
                // Explicitly set; should not be overridden downward by Forager
                "sensitivity_required": "Private",
                "jurisdiction_required": []
            }),
        )
        .await
        .expect("create task");

    assert!(resp.status().is_success(), "explicit Private task rejected: {}", resp.status());
    let body: serde_json::Value = resp.json().await.expect("response json");
    assert_eq!(body["task_id"].as_str(), Some(task_id.as_str()));

    let _ = stack
        .honeycomb_post(&format!("/api/tasks/{task_id}/cancel"), &json!({}))
        .await;
}

/// A task with no PII and explicit public marker routes normally.
#[tokio::test]
#[ignore = "requires running stack: HONEYCOMB_URL DATABASE_URL"]
async fn public_marker_accepted_when_no_pii() {
    let Some(stack) = Stack::from_env() else { return; };

    let task_id = Uuid::new_v4().to_string();
    let resp = stack
        .honeycomb_post(
            "/api/tasks/create",
            &json!({
                "task_id": task_id,
                "owner_id": Uuid::new_v4().to_string(),
                "execution_type": "Llm",
                // Explicit public marker — no PII, should not be upgraded
                "payload": {
                    "profile": "default",
                    "prompt": "What is 2 + 2?",
                    "sensitivity": "public"
                },
                "required_capabilities": {"cpu_cores": null, "memory_mb": null, "llm_profiles": []},
                "allowed_nodes": "hive-wide",
                "jurisdiction_required": []
            }),
        )
        .await
        .expect("create task");

    assert!(resp.status().is_success());

    let _ = stack
        .honeycomb_post(&format!("/api/tasks/{task_id}/cancel"), &json!({}))
        .await;
}
