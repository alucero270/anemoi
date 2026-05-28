# Prompt 00: Lock V1 Scope And No-Database Decision

## Goal

Document Anemoi v1 as a local inference governance daemon with no required
database in phase one.

## Context

Anemoi decides. Runtimes execute.

V1 should prove:

```text
config file in
runtime snapshots in
decision out
explanation out
optional JSONL log out
```

SQLite and database-backed analytics are deferred.

## Required Tests

This is documentation-only unless the repo already contains database-specific
runtime behavior. If database-specific behavior exists, add focused tests that
prove the app can run without a database URL.

Test names, if applicable:

- `daemon_starts_without_database_url`
- `cli_decide_works_without_database_url`
- `decision_log_defaults_to_memory`

## Implementation

Update documentation to state:

- phase one has no required database
- recent decisions are kept in memory
- optional append-only JSONL may persist decisions
- SQLite/database-backed analytics are future work
- `/explain/:id` may only find recent in-memory decisions unless JSONL replay is
  later implemented

Remove phase-one docs that imply SQLite is required.

Do not remove telemetry. Simplify it.

## Acceptance Criteria

- README and contributor docs no longer require SQLite for v1.
- Any database URL is optional.
- No product doc implies SQLite is authoritative.
- Any remaining database mention is clearly marked as future/deferred.

## Validation

Run text safety checks and, if code changed:

```powershell
cargo test --workspace
```

