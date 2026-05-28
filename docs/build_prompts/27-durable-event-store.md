# Prompt 27: Durable Event Store

## Goal

Add a durable event store for decisions, runtime snapshots, resident events,
staging events, action plans, and execution-gate events.

This is the point where a database may become justified.

## Scope

Allowed:

- SQLite-backed event store
- schema migrations
- append-only event records
- query APIs for explain/history
- migration tests

Still not allowed:

- making the database required for basic mock-config operation unless the
  prompt explicitly changes the v1 no-database boundary
- replacing structured decision/explanation records with opaque logs

## Required Tests

Add failing tests first:

- `sqlite_event_store_records_decision_event`
- `sqlite_event_store_records_runtime_snapshot_event`
- `sqlite_event_store_records_staging_event`
- `sqlite_event_store_records_action_plan_event`
- `sqlite_event_store_replays_decision_explanation_by_id`
- `daemon_starts_with_memory_store_when_database_url_is_missing`
- `daemon_uses_sqlite_store_when_database_url_is_present`

## Implementation

Add a telemetry storage abstraction that can support:

- in-memory recent state
- JSONL append-only logs
- SQLite durable events

Suggested event tables:

```text
decisions
runtime_snapshots
resident_events
staging_events
action_plan_events
execution_events
policy_events
```

Use SQLite only when explicitly configured, such as:

```text
ANEMOI_DATABASE_URL=sqlite://...
```

## Acceptance Criteria

- Basic local use still works without a database.
- SQLite gives durable history when configured.
- `/decisions/:id` and `/explain/:id` can read durable decisions.
- Event records preserve enough context to answer why a decision happened.

## Validation

```powershell
cargo fmt --check
cargo test --workspace
```

