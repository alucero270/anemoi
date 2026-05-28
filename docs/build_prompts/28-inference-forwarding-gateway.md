# Prompt 28: Inference Forwarding Gateway

## Goal

Add a `POST /v1/chat/completions` endpoint to `anemoi-daemon` that makes
Anemoi an OpenAI-compatible inference proxy. The caller names a domain as
the model field. Anemoi decides which runtime model to use, forwards the
request to the selected runtime, and streams the response back. The caller
never selects a model directly.

## Context

Prompts 00-20 proved the governance loop: Anemoi inspects runtimes, scores
candidates, and returns a structured decision with explanation. Prompt 28
closes the loop by adding the forwarding layer so applications can treat
Anemoi as a single endpoint without knowing which model is running.

The first real-world use case is opencode. opencode is configured with one
provider (`anemoi`) and one model (`coding`). Every inference request goes to
Anemoi, which governs model selection and forwards transparently.

## Scope

This prompt adds forwarding only. It does not add:

- multi-turn context management
- prompt transformation or templating
- model-specific parameter translation
- load or unload control beyond what prompt 20 defines
- any provider-gateway behavior beyond the selected runtime

## Design

### Endpoint

```
POST /v1/chat/completions
```

Request: standard OpenAI chat completions payload. The `model` field is
treated as a **domain hint**, not a literal model name.

Examples:
- `"model": "coding"` → domain `coding`
- `"model": "anemoi-coding"` → domain `coding` (strip `anemoi-` prefix)
- `"model": "general"` → domain `general`

If the domain is unknown, return a structured error before forwarding.

### Decision flow

```
receive request
  extract domain from model field
  build DecideRequest (domain, mode: interactive, latency_budget_ms: from config or default)
  run policy decide (same path as POST /decide)
  record decision in telemetry
  rewrite model field to selected_model
  forward full request to selected runtime base_url
  stream response back to caller
```

### Forwarding rules

- Forward the original request headers (except Host and Authorization).
- Inject the runtime auth token when the runtime requires it.
- Replace `model` in the forwarded request with `selected_model` from the decision.
- Stream the response body as-is (do not buffer).
- On runtime error or timeout, return a structured error with the decision ID
  so the caller can query `/explain/:id`.

### Safety gate

Forwarding to a non-mock runtime requires `ANEMOI_ENABLE_LIVE_EXECUTE=1`
(established in prompt 20). Mock runtime forwarding is always allowed.

### Response augmentation

Add a response header:

```
X-Anemoi-Decision-Id: <decision-id>
X-Anemoi-Selected-Model: <selected-model>
X-Anemoi-Action: <action>
```

This lets the caller retrieve the explanation without parsing the body.

## Required Tests

Add failing tests first.

Required test names:

- `inference_gateway_maps_model_field_to_domain`
- `inference_gateway_strips_anemoi_prefix_from_model_field`
- `inference_gateway_returns_error_for_unknown_domain`
- `inference_gateway_runs_decide_before_forwarding`
- `inference_gateway_rewrites_model_to_selected_model`
- `inference_gateway_injects_runtime_auth_token`
- `inference_gateway_records_decision_in_telemetry`
- `inference_gateway_returns_decision_id_in_response_header`
- `inference_gateway_requires_live_execute_flag_for_non_mock`
- `inference_gateway_forwards_mock_without_live_execute_flag`
- `inference_gateway_returns_structured_error_on_runtime_failure`

## Implementation

### Crates touched

| Crate | Change |
|---|---|
| `anemoi-daemon` | Add `/v1/chat/completions` handler. Add domain extraction from model field. Add forwarding client. Add response header injection. |
| `anemoi-runtime` | Add `forward_chat_completion` to the adapter trait or as a standalone HTTP client helper. Mock adapter returns a canned streaming response. |
| `anemoi-core` | Add `InferenceRequest` and `InferenceResponse` types if needed. |

### Mock forwarding

The mock runtime adapter must return a deterministic streaming response for
tests. It does not need to call a real model. The response must be a valid
`text/event-stream` SSE payload.

### Streaming

Use `axum`'s `StreamBody` or equivalent. Do not buffer the runtime response.
The forwarding client must support chunked transfer from the runtime.

## Acceptance Criteria

- opencode configured with `baseURL: https://anemoi.home.arpa/v1` and
  `model: coding` receives a valid streaming chat completion response.
- The decision is recorded in telemetry and retrievable via `/explain/:id`.
- Response headers include `X-Anemoi-Decision-Id` and `X-Anemoi-Selected-Model`.
- Non-mock forwarding is blocked without `ANEMOI_ENABLE_LIVE_EXECUTE=1`.
- Unknown domain returns a 400 with a structured explanation before forwarding.
- All 11 required tests pass.

## Validation

```powershell
cargo fmt --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Live smoke after implementation:

```powershell
# Start daemon with live execute enabled
$env:ANEMOI_ENABLE_LIVE_EXECUTE = "1"
$env:ANEMOI_CONFIG = "config/anemoi.prometheus.yaml"
cargo run -p anemoi-daemon

# Send a chat completion through Anemoi
curl -X POST https://anemoi.home.arpa/v1/chat/completions `
  -H "content-type: application/json" `
  -d '{
    "model": "coding",
    "messages": [{"role": "user", "content": "Hello"}],
    "stream": true
  }'

# Retrieve the decision explanation
curl https://anemoi.home.arpa/explain/<X-Anemoi-Decision-Id>
```

## opencode Integration

After this prompt passes, update `opencode.json`:

```json
"anemoi": {
  "npm": "@ai-sdk/openai-compatible",
  "name": "Anemoi",
  "options": {
    "baseURL": "https://anemoi.home.arpa/v1",
    "apiKey": "anemoi"
  },
  "models": {
    "coding": { "name": "Anemoi — Coding" }
  }
}
```

Set `"model": "anemoi/coding"` as the default. opencode sends every request
to Anemoi. Anemoi selects the runtime model, forwards, and streams back.
The model field in the bottom bar reads `anemoi/coding` regardless of which
model is actually serving the request.

## Next Prompt

After this passes: update `docs/test_roadmap.md` to mark prompt 28 Passing
and define prompt 29 for multi-domain support and latency-budget extraction
from request context.
