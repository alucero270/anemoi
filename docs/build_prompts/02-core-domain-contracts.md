# Prompt 02: Core Domain Contracts

## Goal

Harden `anemoi-core` domain types so decisions, explanations, residency states,
and requests have stable serialization contracts.

## Required Tests

Add failing tests first.

Required test names:

- `serializes_residency_state_as_snake_case`
- `serializes_decision_action_as_snake_case`
- `deserializes_interactive_execution_mode`
- `request_id_defaults_to_uuid`
- `decision_explanation_roundtrips_json`
- `score_contributions_preserve_order`
- `runtime_memory_pressure_is_none_without_total`
- `runtime_memory_pressure_calculates_percent`

## Implementation

Keep this prompt inside `anemoi-core`.

Do:

- ensure all public API types serialize predictably
- keep IDs transparent and human-readable
- add helpers only when they reduce repeated logic
- keep defaults explicit

Do not:

- add runtime HTTP logic
- add scheduler scoring
- add telemetry persistence

## Acceptance Criteria

- Core domain objects roundtrip through JSON.
- Public enums use stable snake_case wire values.
- Memory pressure helper behavior is tested.
- No I/O is introduced into `anemoi-core`.

## Validation

```powershell
cargo fmt --check
cargo test -p anemoi-core
```

