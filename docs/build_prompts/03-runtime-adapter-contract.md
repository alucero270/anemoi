# Prompt 03: Runtime Adapter Contract

## Goal

Make the runtime adapter boundary explicit and testable.

## Required Tests

Add failing tests first.

Required test names:

- `adapter_id_is_stable`
- `inspect_returns_normalized_runtime_snapshot`
- `load_model_returns_model_load_handle`
- `execute_returns_execution_handle`
- `unsupported_unload_returns_runtime_error`
- `runtime_errors_are_human_readable`

## Implementation

Work in `anemoi-runtime`.

Ensure the adapter trait remains responsible for:

- inspection
- load handoff
- unload handoff when supported
- execution handoff when supported

Adapters must not:

- score candidates
- select models
- mutate policy config
- hide runtime state behind provider-specific DTOs

## Acceptance Criteria

- Trait behavior is covered through mock implementations.
- Error variants are clear and stable.
- Provider-specific DTOs remain private to adapter modules.
- No scheduler logic moves into `anemoi-runtime`.

## Validation

```powershell
cargo fmt --check
cargo test -p anemoi-runtime
```

