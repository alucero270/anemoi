# Prompt 08: Daemon Decision API

## Goal

Make the local daemon API useful for the first governance loop without full
inference forwarding.

## Required Tests

Add failing tests first.

Required test names:

- `health_returns_ok`
- `status_returns_configured_counts`
- `residents_returns_runtime_snapshots`
- `decide_returns_structured_decision`
- `decide_records_decision_in_log`
- `explain_returns_recorded_explanation`
- `explain_returns_not_found_for_unknown_decision`
- `execute_returns_honest_handoff_response`

## Implementation

Work in `anemoi-daemon`.

Required endpoints:

- `GET /health`
- `GET /status`
- `GET /residents`
- `POST /decide`
- `POST /execute`
- `GET /decisions/:id`
- `GET /explain/:id`

For v1, `/execute` may decide and perform model-load handoff only. It must not
pretend full inference forwarding exists.

## Acceptance Criteria

- API returns structured JSON.
- `/decide` does not execute inference.
- `/execute` behavior is explicit and documented.
- Decisions are available through `/decisions/:id` and `/explain/:id` during
  process lifetime.
- No database is required.

## Validation

```powershell
cargo fmt --check
cargo test -p anemoi-daemon
```

