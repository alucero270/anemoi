# Prompt 25: Eviction And Pinning Policy

## Goal

Add explicit policy for keep-hot groups, pinned models, eviction candidates,
and protected continuity workers.

Eviction should be explainable and dry-run by default.

## Scope

Allowed:

- identify protected residents
- identify eviction candidates
- rank candidates by policy and pressure
- generate eviction plans
- mock-runtime unload tests

Not allowed by default:

- live unload without controlled execution approval
- evicting a keep-hot continuity worker without a strong explicit policy reason

## Required Tests

Add failing tests first:

- `keep_hot_group_members_are_not_evicted_for_background_stage`
- `eviction_plan_prefers_unpinned_idle_resident`
- `eviction_plan_rejects_serving_model_without_force_policy`
- `pinning_policy_explanation_names_protected_model`
- `mock_eviction_executes_unload_action_when_plan_is_approved`
- `live_eviction_requires_explicit_enable_flag`

## Implementation

Extend config with minimal policy fields only when needed:

- group keep-hot behavior
- optional model pinning
- eviction priority or protection

Add an eviction planner that consumes:

- current residents
- pressure model
- requested action plan
- continuity policy

It should produce either:

- no eviction needed
- eviction candidates
- blocked eviction with reasons
- approved mock eviction action

## Acceptance Criteria

- Anemoi can explain why a resident was protected or considered for eviction.
- Keep-hot continuity workers are protected by default.
- Eviction remains dry-run or mock-only unless live execution is explicitly
  enabled.
- Scheduling and action plans can include eviction context without mutating live
  runtimes.

## Validation

```powershell
cargo fmt --check
cargo test --workspace
```

