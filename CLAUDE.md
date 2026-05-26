# CLAUDE.md — hive-gateway-tests

## What this is

Cross-crate integration test suite for the full HiveFabric gateway stack. No library or binary — pure `[[test]]` crates that fire HTTP requests at a running stack and assert on responses. Tests cover the end-to-end path: hive-tenant-gateway → honeycomb → comb node. All tests are `#[ignore]` by default so `cargo test` is clean on machines without a running stack.

## Key files

- `tests/helpers.rs` — `Stack` struct: reads env vars (`HONEYCOMB_URL`, `HONEYCOMB_API_KEY`, `GATEWAY_URL`), provides `honeycomb_get/post`, `gateway_post`, `signup`, `poll_task`. Every test file starts with `Stack::from_env()` — returns `None` and exits silently if vars are missing.
- `tests/smoke.rs` — healthz, self-service signup, LLM provider CRUD, basic task submit, cancel.
- `tests/sensitivity.rs` — Forager PII detection, sensitivity routing hard gate (private tasks only dispatch to private-capable nodes).
- `tests/cancellation.rs` — cancel lifecycle, audit log completeness, 404 on missing task.
- `Cargo.toml` — no `[lib]`; only `[[test]]` entries. Dev deps only (`reqwest`, `serde_json`, `tokio`, `uuid`).

## How to run

```bash
# Start the stack first
cd path/to/honeycomb/docker
docker compose -f compose/docker-compose.with-node.yml up -d

# Set env vars
export HONEYCOMB_URL=http://localhost:8080
export HONEYCOMB_API_KEY=dev-hive-key
export GATEWAY_URL=http://localhost:8090
export DATABASE_URL=postgres://hive:hive@localhost:5432/hive

# Run all integration tests
cargo test -p hive-gateway-tests -- --ignored

# Run a specific suite
cargo test --test smoke -- --ignored
cargo test --test sensitivity -- --ignored
cargo test --test cancellation -- --ignored
```

## Architecture notes

- Tests are stateless by design: each test creates its own tenant via `Stack::signup()` and generates unique `task_id = Uuid::new_v4()`. Never share state across tests.
- `poll_task` polls every 500ms until the task reaches a terminal status (`Succeeded`, `Failed`, `TimedOut`, `Cancelled`) or a configurable timeout.
- `Stack::from_env()` returning `None` is a graceful skip, not a failure. This is intentional for CI without a full stack.
- The suite does not start any services itself — it is purely a black-box HTTP client.

### Required env vars

| Var | Purpose |
|---|---|
| `HONEYCOMB_URL` | Honeycomb base URL (e.g. `http://localhost:8080`) |
| `GATEWAY_URL` | Tenant-gateway base URL (e.g. `http://localhost:8090`) |
| `HONEYCOMB_API_KEY` | Honeycomb x-api-key (default: `dev-hive-key`) |

## CI integration

Add a Postgres service container, start honeycomb and tenant-gateway binaries, then:

```yaml
services:
  postgres:
    image: postgres:16
    env:
      POSTGRES_USER: hive
      POSTGRES_PASSWORD: hive
      POSTGRES_DB: hive
```

Run both services before the test step; set all four env vars; then `cargo test -p hive-gateway-tests -- --ignored`.

## Adding a new test

1. Add `[[test]]` entry to `Cargo.toml` with `path = "tests/<name>.rs"`.
2. Create `tests/<name>.rs`.
3. Begin with `mod helpers; use helpers::Stack; let Some(stack) = Stack::from_env() else { return; };`
4. Mark every `#[tokio::test]` with `#[ignore = "requires running stack: ..."]`.
5. Use `Uuid::new_v4()` for every task id — never hardcode.

## What's not done

- No test for the `/v1/orchestrate` loop (requires a real frontier LLM API key — not suitable for CI without secrets).
- No load/concurrency tests yet.
- Sensitivity suite requires a node registered with `Private` capability; the docker-compose default node does not advertise it — that test suite may skip in default dev stacks.
