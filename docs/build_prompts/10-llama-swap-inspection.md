# Prompt 10: llama-swap Inspection

## Goal

Add real `llama-swap` inspection support without implementing full inference
forwarding.

## Required Tests

Add failing tests first with mocked HTTP responses.

Required test names:

- `llama_swap_health_marks_runtime_available`
- `llama_swap_failed_health_marks_runtime_unavailable`
- `llama_swap_models_response_normalizes_model_ids`
- `llama_swap_inspect_returns_runtime_snapshot`
- `llama_swap_timeout_returns_runtime_error`
- `llama_swap_auth_header_is_applied_when_configured`

## Implementation

Work in `anemoi-runtime` and config types only as needed.

Inspect through documented llama-swap endpoints available in this repo or
runtime docs. If an endpoint cannot prove residency, report `Needs validation`
and expose only the reliable state.

Do not:

- implement chat/completions proxying
- infer hot residency from model list unless the runtime endpoint actually
  means resident/running
- add provider-gateway behavior

## Acceptance Criteria

- Adapter handles available and unavailable runtime states.
- Adapter normalizes reliable runtime state into `RuntimeSnapshot`.
- Ambiguous residency fields are documented as `Needs validation`.
- Tests use local HTTP fixtures/mocks, not live services.

## Validation

```powershell
cargo fmt --check
cargo test -p anemoi-runtime
```

