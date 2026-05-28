# Prompt 21: Runtime Reconciliation Loop

## Goal

Add the first reconciliation loop that repeatedly inspects configured runtimes
and maintains Anemoi's normalized view of availability, residents, active
executions, and ambiguous evidence.

The loop observes. It does not mutate runtime state.

## Scope

Allowed:

- periodic read-only runtime inspection
- cached `RuntimeSnapshot` state
- last-observed timestamp and error state
- status/explanation surfaces that distinguish fresh, stale, failed, and unknown

Not allowed:

- model load
- model unload
- inference execution
- service restart
- hidden mutation as part of reconciliation

## Required Tests

Add failing tests first:

- `reconciliation_loop_updates_snapshot_cache_from_runtime_inspect`
- `reconciliation_loop_marks_snapshot_stale_after_ttl`
- `reconciliation_loop_records_runtime_inspection_error_without_panicking`
- `status_uses_reconciled_snapshot_when_available`
- `decide_uses_reconciled_snapshot_without_reinspecting_when_fresh`

## Implementation

Introduce a small reconciler service in the daemon boundary or a new internal
module.

It should:

- inspect each configured runtime on an interval
- store the latest normalized snapshot per runtime
- store last inspection time
- store last inspection error, if any
- expose cached snapshots to `/status`, `/residents`, `/decide`, and CLI status
- fall back to direct inspection only when no cache is available

Keep the first implementation simple and testable. Avoid distributed locks,
watchers, queues, or multi-node assumptions.

## Acceptance Criteria

- Anemoi has a current observed runtime view without every decision directly
  performing all inspection work.
- Stale data is labeled clearly.
- Failed inspection does not erase the previous snapshot without explanation.
- No mutating runtime action is performed.

## Validation

```powershell
cargo fmt --check
cargo test --workspace
```

