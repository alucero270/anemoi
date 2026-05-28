# Prompt 04: Mock Runtime Snapshots

## Goal

Make `MockRuntimeAdapter` deterministic and expressive enough to support policy
tests without live runtimes.

## Required Tests

Add failing tests first.

Required test names:

- `mock_runtime_starts_with_configured_residents`
- `mock_runtime_load_adds_loading_resident_once`
- `mock_runtime_unload_removes_resident`
- `mock_runtime_execute_records_active_request`
- `mock_runtime_memory_snapshot_is_configurable`
- `mock_runtime_inspect_is_repeatable`

## Implementation

Enhance the mock runtime only as needed for tests.

Useful builder methods:

- `with_memory`
- `with_available`
- `with_resident_state`

Keep behavior deterministic. Avoid sleeping, randomness, background tasks, or
real network calls.

## Acceptance Criteria

- Policy tests can express hot, warm, cold, unavailable, and memory-pressure
  scenarios using the mock runtime.
- Mock state changes are observable through `inspect`.
- No live runtime dependency is introduced.

## Validation

```powershell
cargo fmt --check
cargo test -p anemoi-runtime
```

