//! Shared helpers for the integration test suite.
//!
//! Each test function checks for the required env vars and skips gracefully
//! if they're not set. This keeps `cargo test` clean on developer machines
//! that don't have a running stack.
//!
//! # Running the tests
//!
//! Start the stack (docker compose or local binaries), then:
//!
//! ```sh
//! export HONEYCOMB_URL=http://localhost:8080
//! export HONEYCOMB_API_KEY=dev-hive-key
//! export GATEWAY_URL=http://localhost:8090
//! cargo test -p hive-gateway-tests -- --ignored
//! ```

use reqwest::Client;
use serde_json::Value;
use std::time::Duration;

pub struct Stack {
    pub honeycomb_url: String,
    pub honeycomb_api_key: String,
    pub gateway_url: String,
    pub client: Client,
}

impl Stack {
    /// Returns `None` and prints a skip message if required env vars are missing.
    pub fn from_env() -> Option<Self> {
        let honeycomb_url = std::env::var("HONEYCOMB_URL").ok()?;
        let honeycomb_api_key =
            std::env::var("HONEYCOMB_API_KEY").unwrap_or_else(|_| "dev-hive-key".to_string());
        let gateway_url = std::env::var("GATEWAY_URL").ok()?;
        Some(Self {
            honeycomb_url,
            honeycomb_api_key,
            gateway_url,
            client: Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("client"),
        })
    }

    pub async fn honeycomb_get(&self, path: &str) -> reqwest::Result<reqwest::Response> {
        self.client
            .get(format!("{}{}", self.honeycomb_url, path))
            .header("x-api-key", &self.honeycomb_api_key)
            .send()
            .await
    }

    pub async fn honeycomb_post(&self, path: &str, body: &Value) -> reqwest::Result<reqwest::Response> {
        self.client
            .post(format!("{}{}", self.honeycomb_url, path))
            .header("x-api-key", &self.honeycomb_api_key)
            .json(body)
            .send()
            .await
    }

    /// Sign up a new tenant on the gateway. Returns (tenant_id, api_key).
    pub async fn signup(&self, name: &str) -> (String, String) {
        let resp = self
            .client
            .post(format!("{}/v1/signup", self.gateway_url))
            .json(&serde_json::json!({"name": name}))
            .send()
            .await
            .expect("signup request");
        assert!(resp.status().is_success(), "signup failed: {:?}", resp.status());
        let body: Value = resp.json().await.expect("signup response json");
        let tenant_id = body["tenant_id"].as_str().expect("tenant_id").to_string();
        let api_key = body["api_key"].as_str().expect("api_key").to_string();
        (tenant_id, api_key)
    }

    pub async fn gateway_post(&self, path: &str, api_key: &str, body: &Value) -> reqwest::Result<reqwest::Response> {
        self.client
            .post(format!("{}{}", self.gateway_url, path))
            .header("authorization", format!("bearer {api_key}"))
            .json(body)
            .send()
            .await
    }

    /// Poll a honeycomb task until it reaches a terminal status or times out.
    pub async fn poll_task(&self, task_id: &str, timeout_secs: u64) -> Value {
        let deadline = std::time::Instant::now() + Duration::from_secs(timeout_secs);
        loop {
            tokio::time::sleep(Duration::from_millis(500)).await;
            let resp = self
                .honeycomb_get(&format!("/api/tasks/{task_id}"))
                .await
                .expect("poll request");
            if resp.status().is_success() {
                let task: Value = resp.json().await.expect("task json");
                let status = task["status"].as_str().unwrap_or("");
                match status {
                    "Succeeded" | "Failed" | "TimedOut" | "Cancelled" => return task,
                    _ => {}
                }
            }
            assert!(
                std::time::Instant::now() < deadline,
                "task {task_id} did not complete within {timeout_secs}s"
            );
        }
    }
}
