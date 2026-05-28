# Prompt 26: Operator Status Surface

## Goal

Make Anemoi feel like an operator-facing control plane rather than a collection
of JSON endpoints.

This does not require a full web UI. A rich CLI/status surface is acceptable for
this prompt.

## Supplemental Context

Read **GitHub issue #17** (Diagnostics endpoint and route visibility) alongside
this prompt. Issue #17 defines the consolidated diagnostics view that this
prompt implements. Key additions:

- `GET /status` must return all governance state in a single call — runtime
  availability, snapshot freshness, residents by domain/group, staging queue
  summary, recent decision count, policy warnings, and live execution gate state
- All data comes from the reconciled snapshot cache (prompt 21); this endpoint
  must not trigger live inspection
- Stale and unknown states must be labeled explicitly — never silently omitted
  or represented as empty arrays

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
- snapshot freshness (from prompt 21 reconciliation cache)
- residents by runtime
- residents by group when known
- active requests
- staging queue summary (from prompt 22)
- recent decision count
- policy warnings (e.g., keep-hot group has no hot residents)
- live execution gate state (`ANEMOI_ENABLE_LIVE_EXECUTE` on/off)
- unknown / stale labels wherever data is missing or aged

Expose through:

- `GET /status` — structured JSON
- `anemoi status` — human-readable table

Keep raw `/residents` available for machine-readable detail.

## Acceptance Criteria

- A human can tell what Anemoi thinks is resident and why from a single
  `GET /status` or `anemoi status` call.
- Stale, failed, and unknown states are visible and labeled — never silently
  omitted.
- CLI status output is useful without reading raw JSON.
- Existing API compatibility is preserved or versioned intentionally.

## Dependencies

- Prompt 21 (runtime reconciliation loop) — status uses reconciled snapshot cache
- Prompt 22 (background staging worker) — staging queue summary in status
- GitHub issue #17 — consolidated diagnostics view design

## Validation

```powershell
cargo fmt --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```
