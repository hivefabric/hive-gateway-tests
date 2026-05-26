# hive-gateway-tests

Cross-crate integration test suite for the HiveFabric gateway stack.

Covers the full dispatch loop across service boundaries:
- `hive-tenant-gateway` → `honeycomb` → comb node (end-to-end)
- Sensitivity routing (Forager classifier + scheduler hard gate)
- Task cancellation lifecycle + audit log

---

## Running

All tests are `#[ignore]` by default — they require a running stack.

```bash
# Start the stack
cd path/to/honeycomb/docker
docker compose -f compose/docker-compose.with-node.yml up -d

# Set env vars
export HONEYCOMB_URL=http://localhost:8080
export HONEYCOMB_API_KEY=dev-hive-key
export GATEWAY_URL=http://localhost:8090
export DATABASE_URL=postgres://hive:hive@localhost:5432/hive

# Run all integration tests
cargo test -- --ignored

# Run a specific suite
cargo test --test smoke -- --ignored
cargo test --test sensitivity -- --ignored
cargo test --test cancellation -- --ignored
```

---

## Test suites

| File | What it tests |
|---|---|
| `smoke.rs` | Healthz, self-service signup, LLM provider CRUD, basic task submit, cancel |
| `sensitivity.rs` | Forager PII detection, sensitivity routing hard gate |
| `cancellation.rs` | Cancel lifecycle, audit log completeness, 404 on missing task |

---

## Adding a test

1. Add a new `[[test]]` entry to `Cargo.toml`
2. Create `tests/<name>.rs`
3. Start with `mod helpers; use helpers::Stack; let Some(stack) = Stack::from_env() else { return; };`
4. Mark every test `#[ignore = "requires running stack: ..."]`
5. Keep tests independent — use unique `task_id = Uuid::new_v4()` for every task

---

## CI integration

To run in CI, add a service container for Postgres and start the stack:

```yaml
services:
  postgres:
    image: postgres:16
    env:
      POSTGRES_USER: hive
      POSTGRES_PASSWORD: hive
      POSTGRES_DB: hive
```

Then run honeycomb and tenant-gateway binaries before the test step.
