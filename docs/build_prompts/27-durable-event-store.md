# Prompt 27: Durable Event Store

## Goal

Add a durable event store for decisions, runtime snapshots, resident events,
staging events, action plans, and execution-gate events.

This is the point where a database may become justified.

## Supplemental Context

Read **GitHub issue #12** (Transition records for resident events) alongside
this prompt. Issue #12 defines the `resident_events` table schema and
requirements. Key additions:

The `resident_events` table must capture:
- resident ID (model + runtime)
- from state (using established `ResidencyState` vocabulary)
- to state (using established `ResidencyState` vocabulary)
- observed at (timestamp)
- evidence source — which adapter, which inspection round (never anonymous)
- decision ID that triggered the transition, if known
- note for ambiguous transitions

Rules from issue #12:
- Transitions without a triggering decision are recorded as observation-only
- Do not invent `ResidencyState` variants to represent transitions — use the
  established vocabulary (`cold`, `loading`, `warm_cpu`, `partial`, `hot_gpu`,
  `serving`, `draining`, `evicting`, `failed`)
- Transition records are append-only — no update or delete paths

## Scope

Allowed:

- SQLite-backed event store when `ANEMOI_DATABASE_URL` is set
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

Event tables:

```text
decisions
runtime_snapshots
resident_events          ← see issue #12 for schema
staging_events
action_plan_events
execution_events
policy_events
```

Use SQLite only when explicitly configured:

```text
ANEMOI_DATABASE_URL=sqlite:///var/lib/anemoi/events.db
```

## Acceptance Criteria

- Basic local use still works without a database.
- SQLite gives durable history when configured.
- `/decisions/:id` and `/explain/:id` can read durable decisions after daemon
  restart.
- Event records preserve enough context to answer why a decision happened.
- Resident state transitions are captured in `resident_events` with evidence
  source always recorded.

## Dependencies

- Prompt 22 (background staging worker) — staging events
- Prompt 23 (load/unload action plan) — action plan events
- Prompt 21 (runtime reconciliation loop) — snapshot events
- GitHub issue #12 — resident_events table schema

## Validation

```powershell
cargo fmt --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```
