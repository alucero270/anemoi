# Prompt 19: Live Decision Smoke

## Goal

Run the first read-only live decision smoke using real runtime inspection and
Anemoi's scheduler.

## Scope

Read-only runtime validation.

Allowed:

- inspect runtime
- build `RuntimeSnapshot`
- run `/decide`
- run CLI `decide`
- record decision and explanation

Not allowed:

- `/execute` against live runtime
- model load/unload
- inference forwarding
- service restart
- config mutation

## Required Inputs

- validated runtime config profile from prompt 17
- residency truth contract from prompt 18
- runtime base URL
- auth handling
- small worker model id
- large target model id
- latency budget

## Required Tests

Add failing tests first for any new smoke harness behavior.

Required test names if code changes:

- `live_smoke_decide_uses_runtime_snapshot_without_execute`
- `live_smoke_decision_records_runtime_evidence_source`
- `live_smoke_explanation_includes_latency_and_residency_reasons`

## Implementation

Create a documented smoke procedure.

The smoke should capture:

- command run
- sanitized config used
- runtime snapshot
- decision response
- explanation response
- whether small-worker fallback occurred
- whether background staging was recommended
- limitations and `Needs validation`

Do not require the smoke to pass if the live runtime does not expose resident
state. In that case, record the gap honestly.

## Acceptance Criteria

- An operator can reproduce the read-only smoke.
- The smoke proves what Anemoi can decide from real runtime state.
- If runtime evidence is insufficient, the result is recorded as `Needs
  validation`, not hidden.
- No mutating live action is performed.

## Validation

```powershell
cargo fmt --check
cargo test --workspace
```

Record live smoke commands and results separately after explicit approval.

