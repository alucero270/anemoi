# Prompt 22: Background Staging Worker

## Goal

Turn `StageBackground` from an explanation-only decision into a queued,
observable staging intent.

This prompt may create staging jobs, but live runtime mutation must remain
disabled unless Prompt 20's controlled execution gate has explicitly enabled it.

## Scope

Default behavior:

```text
decision recommends staging
staging intent is recorded
worker reports it as blocked until live execution is explicitly enabled
```

Allowed by default:

- enqueue staging intent
- inspect staging queue
- mark intent blocked, pending, skipped, failed, or completed in mock tests
- execute staging against mock runtime

Not allowed by default:

- live model load
- live model unload
- live inference request

## Required Tests

Add failing tests first:

- `stage_background_decision_enqueues_staging_intent`
- `staging_worker_does_not_mutate_live_runtime_without_enable_flag`
- `staging_worker_can_load_model_on_mock_runtime`
- `staging_status_reports_pending_blocked_failed_and_completed`
- `staging_intent_records_decision_id_model_runtime_and_reason`

## Implementation

Add a staging queue and worker abstraction.

The first version can be in-memory. It should record:

- staging intent id
- decision id
- selected foreground model
- background model
- target runtime
- reason
- created time
- current state
- last error

Expose staging state through one or more operator surfaces:

- `GET /staging`
- `GET /status`
- `anemoi staging`

Mock runtime staging may actually call `load_model`. Live runtime staging must
remain gated.

## Acceptance Criteria

- Background staging is represented as an explicit product object.
- Operators can see whether staging was recommended, blocked, pending, failed,
  or completed.
- Live runtime mutation cannot happen by accident.
- Explanations connect decisions to staging intents.

## Validation

```powershell
cargo fmt --check
cargo test --workspace
```

