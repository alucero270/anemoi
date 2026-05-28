# Controlled Execution Gate

## Purpose

Define the approval gate for any live execution validation that involves model
load, unload, staging, or inference handoff. This gate prevents accidental live
runtime mutation.

## Default State

Live execution is **disabled by default**.

| Endpoint | Default Behavior |
|---|---|
| `/decide` | Always allowed (read-only decision). |
| `/execute` (mock runtime) | Always allowed (safe for testing). |
| `/execute` (non-mock runtime) | Blocked unless `ANEMOI_ENABLE_LIVE_EXECUTE=1` is set. |

## Opt-In

To enable live execution against non-mock runtimes:

```powershell
$env:ANEMOI_ENABLE_LIVE_EXECUTE = "1"
```

This must be set before starting the daemon. The flag applies process-wide.

## Approval Checklist

Before any live mutating validation, the operator or maintainer must approve:

| Item | Required |
|---|---|
| Runtime target | Yes |
| Exact endpoint or command | Yes |
| Model id | Yes |
| Expected load/unload/execution behavior | Yes |
| Timeout | Yes |
| Rollback or recovery plan | Yes |
| Observation method | Yes |
| Success criteria | Yes |
| Stop conditions | Yes |

## Execution Paths

### Read-Only (always allowed)
- `GET /health`
- `GET /status`
- `GET /residents`
- `POST /decide`
- `GET /decisions/:id`
- `GET /explain/:id`

### Controlled (mock only, no flag required)
- `POST /execute` against a runtime with `adapter: mock`
- Runtime load/unload through mock adapter

### Gated (requires `ANEMOI_ENABLE_LIVE_EXECUTE=1`)
- `POST /execute` against a non-mock runtime
- Runtime model load through a non-mock adapter

### Future (not in v1)
- Full inference forwarding
- Runtime model unload
- Runtime config mutation

## Rollback

If a live execution test produces unexpected behavior:

1. Stop the daemon process.
2. Verify no models were left in an unexpected state.
3. If the runtime was mutated, reload the runtime or restore from backup config.
4. Record the unexpected behavior in `docs/live_validation/` as `Needs validation`.
5. Do not re-enable live execution until the root cause is understood.

## Decision Recording

When live execution is enabled and a mutating action occurs:

- The decision is recorded in the decision log.
- The explanation includes the runtime target and model id.
- The handoff response reports `load_requested: true` when load was attempted.

## Related Documents

- [Safety Plan](safety-plan.md)
- [Residency Truth Contract](residency-truth-contract.md)
- [Decision Smoke Procedure](decision-smoke.md)
