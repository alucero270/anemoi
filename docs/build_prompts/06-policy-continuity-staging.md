# Prompt 06: Continuity Staging Policy

## Goal

Fully specify and test the first proof-of-value behavior: use a hot acceptable
worker now and stage a larger cold model in the background when policy allows.

## Required Tests

Add failing tests first.

Required test names:

- `avoids_cold_large_model_when_small_worker_is_hot`
- `does_not_stage_background_when_policy_disallows_background_load`
- `does_not_stage_background_when_latency_budget_allows_cold_load`
- `does_not_stage_background_without_hot_fallback`
- `records_background_model_in_decision`
- `explanation_names_selected_and_staged_models`
- `score_includes_continuity_contribution`

## Implementation

Work in `anemoi-policy`.

The policy should stage only when:

- a cold candidate exceeds the latency budget
- a hot or warm acceptable fallback exists
- continuity policy allows background loading
- policy prefers degraded response over silence

Every continuity decision must explain:

- selected model
- staged model
- latency budget
- cold load estimate
- continuity policy reason

## Acceptance Criteria

- Stage behavior is deterministic and fully explained.
- No background model is recorded unless staging is actually selected.
- Deny/defer/load/reuse behavior remains intact.

## Validation

```powershell
cargo fmt --check
cargo test -p anemoi-policy
```

