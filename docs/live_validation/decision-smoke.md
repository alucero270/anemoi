# Live Decision Smoke Procedure

## Permission Boundary

Read-only runtime validation. Allowed:

- inspect runtime (HTTP GET only)
- build `RuntimeSnapshot`
- run `/decide` (does not execute inference)
- run CLI `decide` (does not execute inference)
- record decision and explanation

Not allowed:

- `/execute` against live runtime
- model load/unload
- inference forwarding
- service restart
- config mutation

## Required Inputs

| Input | Source |
|---|---|
| Runtime config profile | `config/anemoi.llama-swap.example.yaml` |
| Residency truth contract | `docs/live_validation/residency-truth-contract.md` |
| Runtime base URL | `ANEMOI_LLAMA_SWAP_BASE_URL` env var |
| Auth handling | `ANEMOI_LLAMA_SWAP_AUTH_TOKEN` env var (optional) |
| Small worker model id | `qwen9b` |
| Large target model id | `qwen35_a3b` |
| Latency budget | `1500ms` |

## Smoke Steps

### 1. Start daemon with live config

```powershell
$env:ANEMOI_CONFIG = "config/anemoi.llama-swap.example.yaml"
$env:ANEMOI_BIND = "127.0.0.1:7070"
cargo run -p anemoi-daemon
```

### 2. Check health

```powershell
curl http://127.0.0.1:7070/health
```

Expected: `{"ok":true}`

### 3. Inspect runtime

```powershell
curl http://127.0.0.1:7070/status
curl http://127.0.0.1:7070/residents
```

Expected: Status shows 1 runtime. Residents shows the llama-swap snapshot.

### 4. Run read-only decision

```powershell
curl -X POST http://127.0.0.1:7070/decide `
  -H "content-type: application/json" `
  -d '{\"domain\":\"coding\",\"mode\":\"interactive\",\"latency_budget_ms\":1500}'
```

### 5. Record explanation

```powershell
# Copy the decision id from step 4 output
curl http://127.0.0.1:7070/explain/<decision-id>
```

## CLI Alternative

```powershell
cargo run -p anemoi-cli -- --config config/anemoi.llama-swap.example.yaml decide --domain coding --latency-budget-ms 1500
```

## Evidence Collection

Capture for each smoke run:

| Field | Value |
|---|---|
| Timestamp | `TBD` |
| Sanitized config used | `TBD` |
| Runtime snapshot | `TBD` |
| Decision response | `TBD` |
| Explanation response | `TBD` |
| Small-worker fallback occurred? | `TBD` |
| Background staging recommended? | `TBD` |
| Limitations and Needs validation | `TBD` |

## Success Criteria

1. All commands complete without error.
2. Decision selects the small hot worker (if hot) or falls back gracefully.
3. Explanation includes latency budget and residency reasons.
4. Background staging recommendation is present if large model was rejected.
5. No mutating action was performed.

## Limitations

- If the live runtime does not expose resident state, the smoke documents the
  gap honestly as `Needs validation`.
- This is a read-only smoke. `/execute` is not tested.
- JSONL decision log is not collected unless `ANEMOI_DECISION_LOG` is set.
