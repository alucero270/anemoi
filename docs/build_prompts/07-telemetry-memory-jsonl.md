# Prompt 07: Telemetry Without A Database

## Goal

Remove SQLite as a phase-one requirement. Keep telemetry simple: in-memory
recent decisions plus optional append-only JSONL.

## Required Tests

Add failing tests first.

Required test names:

- `memory_decision_log_stores_and_gets_decision`
- `memory_decision_log_returns_none_for_unknown_decision`
- `memory_decision_log_keeps_recent_decisions_in_insert_order`
- `jsonl_decision_log_appends_one_json_object_per_decision`
- `jsonl_decision_log_creates_parent_directory_when_needed`
- `jsonl_decision_log_does_not_require_sqlite`
- `telemetry_trait_supports_memory_and_jsonl_logs`

## Implementation

Work in `anemoi-telemetry`, `anemoi-daemon`, and `anemoi-cli` only as needed.

Replace required database behavior with:

- `InMemoryDecisionLog`
- optional `JsonlDecisionLog`
- a trait shared by daemon and CLI

Suggested environment variable:

```text
ANEMOI_DECISION_LOG=logs/anemoi-decisions.jsonl
```

Remove phase-one `ANEMOI_DATABASE_URL` behavior unless retained as explicitly
deferred/disabled compatibility.

Do not add SQLite migrations, schemas, or query APIs.

## Acceptance Criteria

- Anemoi runs without any database URL.
- Decisions can be retrieved from memory during process lifetime.
- Optional JSONL writes valid one-line JSON per decision.
- Docs no longer present SQLite as required for v1.

## Validation

```powershell
cargo fmt --check
cargo test --workspace
```

