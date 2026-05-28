# Prompt 23: Load/Unload Action Plan

## Goal

Introduce an explicit action plan between policy decisions and runtime
mutation.

Anemoi should be able to say what it would do before it does it.

## Scope

Allowed:

- generate load, unload, keep, stage, defer, deny, and no-op actions
- dry-run plans
- mock-runtime execution of plans
- explicit live-runtime blocking unless enabled by Prompt 20

Not allowed:

- automatic live mutation without an approved gate
- implicit load/unload inside `/execute` without an action plan record

## Required Tests

Add failing tests first:

- `decision_action_plan_contains_foreground_load_when_required`
- `stage_background_action_plan_contains_background_load_intent`
- `reuse_hot_action_plan_contains_no_mutating_action`
- `action_plan_dry_run_does_not_call_runtime_adapter`
- `live_action_plan_execution_requires_explicit_enable_flag`
- `action_plan_explanation_lists_each_planned_action`

## Implementation

Add a structured action plan model in `anemoi-core`, for example:

```rust
pub struct ActionPlan {
    pub decision_id: DecisionId,
    pub actions: Vec<RuntimeAction>,
    pub dry_run: bool,
}
```

Runtime actions should include enough metadata to audit:

- action kind
- runtime id
- model id, if applicable
- foreground or background
- mutating or read-only
- policy reason
- expected cost

Update `/execute` so it creates and reports an action plan. Default live
behavior remains dry-run or blocked.

## Acceptance Criteria

- There is no hidden runtime mutation path.
- `/execute` becomes decision plus explicit action plan plus handoff result.
- Mock-runtime action execution is test-covered.
- Live-runtime action execution is blocked unless explicitly enabled.

## Validation

```powershell
cargo fmt --check
cargo test --workspace
```

