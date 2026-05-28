# Live Validation Safety Plan

## Permission Boundary

Read-only inspection only. The following actions are **never allowed** without
explicit user approval and a documented rollback plan:

- model load
- model unload
- service restart
- config edit
- inference execution
- secrets committed to Git

## Required Operator Inputs

Before any live validation, collect and confirm:

| Input | Example | Status |
|---|---|---|
| Runtime target | `llama-swap`, Ollama, or llama.cpp | `Needs validation` |
| Runtime base URL | `http://127.0.0.1:8085` | `Needs validation` |
| Auth header or API key handling | `Authorization: Bearer <redacted>` | `Needs validation` |
| Small continuity worker model id | `qwen3.5-9b` or `granite-4.1-8b` | `Needs validation` |
| Large staged model id | `qwen3.6-35b-a3b` | `Needs validation` |
| Expected domain | `coding` | `Needs validation` |
| Latency budget | `1500ms` | `Needs validation` |
| Operator success criteria | See below | `Needs validation` |
| Rollback expectations | See below | `Needs validation` |

## Read-Only Commands To Collect Current State

Use environment variables for sensitive values:

```powershell
$env:ANEMOI_CONFIG = "config/anemoi.example.yaml"
$env:ANEMOI_BIND = "127.0.0.1:7070"
$env:ANEMOI_LLAMA_SWAP_BASE_URL = "http://127.0.0.1:8085"
$env:ANEMOI_LLAMA_SWAP_AUTH_HEADER = "Authorization: Bearer <redacted>"
```

### Health check
```powershell
curl http://127.0.0.1:7070/health
```

### Runtime status
```powershell
cargo run -p anemoi-cli -- status
cargo run -p anemoi-cli -- runtimes
```

### Resident snapshots
```powershell
cargo run -p anemoi-cli -- residents
```

### Read-only decision
```powershell
cargo run -p anemoi-cli -- decide --domain coding --latency-budget-ms 1500
```

## Where To Record Observations

Record all observations in `docs/live_validation/` with:

- timestamp
- runtime base URL used
- sanitized response shapes (no secrets)
- model ids returned
- Anemoi interpretation
- unexpected findings
- `Needs validation` items

## Stop Conditions

Stop immediately and record the issue when:

- any endpoint returns unexpected errors
- runtime evidence contradicts documented semantics
- a probe triggers more than read-only behavior
- auth tokens or secrets are exposed
- ambiguity cannot be resolved without mutation

## Success Criteria

A live validation session succeeds when:

1. All read-only commands complete without error.
2. Runtime snapshot contains the expected model ids.
3. Decision smoke produces a valid decision with explanation.
4. Findings are recorded without secrets.
5. No unintended mutation occurred.
6. Remaining unknowns are marked `Needs validation`.

## Next Prompt

After this plan is documented and approved, proceed to
[Prompt 16: llama-swap Read-Only Probe](../build_prompts/16-llama-swap-readonly-probe.md)
to execute the first read-only probe.
