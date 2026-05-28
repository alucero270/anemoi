# Prompt 16: llama-swap Read-Only Probe

## Goal

Validate what live `llama-swap` exposes through read-only endpoints and compare
that evidence to Anemoi's normalized `RuntimeSnapshot`.

## Scope

Read-only only.

Allowed:

- HTTP `GET` requests to health/model/status endpoints
- reading sanitized local docs
- reading logs only if explicitly approved and no secrets are exposed

Not allowed:

- model load
- model unload
- inference request
- service restart
- config edit
- live file modification

## Required Inputs

- `ANEMOI_LLAMA_SWAP_BASE_URL`
- optional `ANEMOI_LLAMA_SWAP_AUTH_HEADER`
- expected configured model ids
- expected small-worker model id
- expected large-model id

Use environment variables in examples:

```powershell
$env:ANEMOI_LLAMA_SWAP_BASE_URL = "http://127.0.0.1:8085"
$env:ANEMOI_LLAMA_SWAP_AUTH_HEADER = "Authorization: Bearer <redacted>"
```

## Required Tests

Add failing fixture tests first if adapter behavior changes.

Required test names:

- `llama_swap_probe_does_not_require_mutating_endpoint`
- `llama_swap_probe_records_unknown_residency_when_endpoint_is_ambiguous`
- `llama_swap_probe_maps_configured_models_without_claiming_hot_residency`

## Implementation

Create a documented read-only probe procedure and, if useful, a CLI command or
test helper that performs only safe inspection.

The probe should capture:

- endpoint URL
- HTTP status
- sanitized response shape
- model ids returned
- whether the endpoint proves configured, loaded, resident, or unknown state
- timestamp
- exact Anemoi interpretation

Do not treat `/v1/models` as proof of hot residency unless live evidence proves
that meaning.

## Acceptance Criteria

- The probe produces a clear evidence table.
- Ambiguous endpoints result in `unknown` or configured-only state, not false
  hot residency.
- No mutating endpoint is called.
- Findings are recorded in docs.

## Validation

```powershell
cargo fmt --check
cargo test --workspace
```

Live read-only commands may be reported separately only after explicit approval.

