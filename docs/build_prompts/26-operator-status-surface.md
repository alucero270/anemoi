# Prompt 26: Operator Status Surface

## Goal

Make Anemoi feel like an operator-facing control plane rather than a collection
of JSON endpoints.

This does not require a full web UI. A rich CLI/status surface is acceptable for
this prompt.

## Scope

Allowed:

- richer `/status` response
- CLI status table
- residents grouped by runtime and residency group
- decision/staging/action summaries
- stale and unknown state labels

Not required:

- browser dashboard
- authentication
- multi-user UI

## Required Tests

Add failing tests first:

- `status_summary_includes_runtime_availability_and_staleness`
- `status_summary_includes_residency_group_health`
- `status_summary_includes_recent_decision_count`
- `cli_status_prints_residents_staging_and_policy_summary`
- `cli_status_marks_unknown_and_stale_state_plainly`

## Implementation

Create an operator summary model. It should include:

- runtime availability
- snapshot freshness
- residents by runtime
- residents by group when known
- active requests
- staging queue summary
- recent decision count
- policy warnings
- live execution gate state

Expose it through:

- `GET /status`
- `anemoi status`

Keep raw `/residents` available for machine-readable details.

## Acceptance Criteria

- A human can tell what Anemoi thinks is resident and why.
- Stale, failed, and unknown states are visible.
- CLI status output is useful without reading raw JSON.
- Existing API compatibility is preserved or versioned intentionally.

## Validation

```powershell
cargo fmt --check
cargo test --workspace
```

