# llama.cpp Read-Only Probe Procedure

This procedure validates the `LlamaCppAdapter` (adapter id `llama_cpp` /
`llama_server`) against a live `llama-server` instance. The adapter inspects
only — it never loads, unloads, or executes.

## Permission Boundary

Read-only HTTP GET requests only. No POST, PUT, DELETE, or PATCH. This probe is
covered by the read-only phase of [`safety-plan.md`](safety-plan.md) and does
not require `ANEMOI_ENABLE_LIVE_EXECUTE=1`.

## Required Inputs

Set these environment variables before running the probe:

```powershell
$env:ANEMOI_LLAMA_CPP_BASE_URL = "http://127.0.0.1:8080"
# Only if the endpoint requires authentication:
$env:ANEMOI_LLAMA_CPP_AUTH_TOKEN = "<redacted>"
```

The auth token is read from the environment and expanded at config load. It is
never committed.

## Probe Steps

### 1. Health Endpoint

```powershell
curl $env:ANEMOI_LLAMA_CPP_BASE_URL/health
```

Expected: HTTP 200. Confirms `llama-server` is reachable.

### 2. Models Endpoint

```powershell
curl $env:ANEMOI_LLAMA_CPP_BASE_URL/v1/models
```

Expected: JSON with a `data` array of model objects. Each model has an `id`
field.

Note: `/v1/models` proves the model is **configured/served by the process**, not
that Anemoi has independent evidence of GPU residency. Per the
[residency truth contract](residency-truth-contract.md), Anemoi surfaces these
ids as `configured_models` and does not claim hot residency.

### 3. Anemoi Runtime Inspect

```powershell
cargo run -p anemoi-cli -- runtimes
cargo run -p anemoi-cli -- residents
```

Expected: Runtime is listed with adapter `llama_cpp`. The `residents` array is
empty (no false hot claims); configured models are surfaced separately.

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
| Health returns 200, /v1/models returns models | Runtime is available. Models are configured. Residency is unknown (empty residents). |
| Health returns non-200 | Runtime is unavailable. Empty residents and configured models. |
| /v1/models returns empty data | Runtime is available with no configured models. |
| /v1/models is not reachable but health is OK | `inspect()` returns an HTTP error; residency is unknown. |

Do not claim hot residency unless a runtime-specific endpoint proves a model is
currently loaded and serving.

## Status

**Needs validation.** Fixture tests in `crates/anemoi-runtime` cover the parsing
and normalization contract. A live `llama-server` run against this procedure has
not yet been recorded.
