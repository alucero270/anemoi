# Prompt 11: Ollama Inspection

## Goal

Add reliable Ollama inspection support for running/resident models.

## Required Tests

Add failing tests first with mocked HTTP responses.

Required test names:

- `ollama_ps_response_maps_running_models_to_hot_residents`
- `ollama_ps_empty_response_returns_no_residents`
- `ollama_ps_vram_bytes_convert_to_mb`
- `ollama_unavailable_runtime_returns_error_or_unavailable_snapshot`
- `ollama_malformed_response_returns_runtime_error`
- `ollama_base_url_validation_rejects_invalid_url`

## Implementation

Work in `anemoi-runtime`.

Use Ollama's running-model inspection endpoint already represented by the
existing adapter behavior. Keep DTOs private to the adapter.

Do not add generation or chat forwarding in this prompt.

## Acceptance Criteria

- Running Ollama models become `ModelResident` entries.
- State is normalized as `hot_gpu` only when the endpoint indicates the model
  is currently running/resident.
- Byte-to-MB conversion is tested.
- Adapter errors are clear.

## Validation

```powershell
cargo fmt --check
cargo test -p anemoi-runtime
```

