# llama-swap Read-Only Probe Procedure

## Permission Boundary

Read-only HTTP GET requests only. No POST, PUT, DELETE, or PATCH.

## Required Inputs

Set these environment variables before running the probe:

```powershell
$env:ANEMOI_LLAMA_SWAP_BASE_URL = "http://127.0.0.1:8085"
# Only if the endpoint requires authentication:
$env:ANEMOI_LLAMA_SWAP_AUTH_HEADER = "Authorization: Bearer <redacted>"
```

## Probe Steps

### 1. Health Endpoint

```powershell
curl $env:ANEMOI_LLAMA_SWAP_BASE_URL/health
```

Expected: HTTP 200. Confirms runtime is reachable.

### 2. Models Endpoint

```powershell
curl $env:ANEMOI_LLAMA_SWAP_BASE_URL/v1/models
```

Expected: JSON with a `data` array of model objects. Each model has an `id`
field.

Note: `/v1/models` proves models are **configured**, not that they are
**resident** or **hot**. Anemoi does not claim hot residency from this endpoint.

### 3. Anemoi Runtime Inspect

```powershell
cargo run -p anemoi-cli -- runtimes
cargo run -p anemoi-cli -- residents
```

Expected: Runtime is listed with adapter `llama_swap`. Residents array is empty
(no false hot claims).

## Evidence Table

Capture this information for each probe run:

| Field | Value |
|---|---|
| Timestamp | `TBD` |
| Runtime base URL | `TBD` |
| Health HTTP status | `TBD` |
| /v1/models response count | `TBD` |
| Model IDs returned | `TBD` |
| Anemoi interpretation | `TBD` |
| Unexpected findings | `TBD` |
| Needs validation | `TBD` |

## Interpretation Rules

| Endpoint Evidence | Anemoi Interpretation |
|---|---|
| Health returns 200, /v1/models returns models | Runtime is available. Models are configured. Residency is unknown. |
| Health returns non-200 | Runtime is unavailable. |
| /v1/models returns empty data | Runtime has no configured models. |
| /v1/models is not reachable but health is OK | Runtime is degraded. Residency is unknown. |

Do not claim hot residency unless a runtime-specific endpoint proves a model is
currently loaded and serving.

## Next Prompt

After the probe document and tests are accepted, proceed to
[Prompt 17: Live Runtime Config Profile](../build_prompts/17-live-runtime-config-profile.md).
