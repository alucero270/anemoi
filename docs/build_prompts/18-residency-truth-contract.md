# Prompt 18: Residency Truth Contract

## Goal

Define exactly what evidence allows Anemoi to report a model as `cold`,
`loading`, `warm_cpu`, `partial`, `hot_gpu`, `serving`, `draining`, `evicting`,
or `failed`.

## Context

False residency claims are worse than unknown state. Anemoi must not say a model
is hot just because it is configured.

## Required Tests

Add failing tests first.

Required test names:

- `configured_model_without_runtime_residency_evidence_is_not_hot`
- `running_model_endpoint_maps_to_hot_or_serving`
- `failed_runtime_health_maps_to_unavailable_snapshot`
- `ambiguous_runtime_state_preserves_unknown_or_cold_candidate_reason`
- `decision_explanation_mentions_ambiguous_residency_evidence`

## Implementation

Add a contract document and adjust adapter normalization if needed.

The contract should cover:

- configured model
- available model
- loaded model
- resident model
- active/serving model
- failed or unreachable runtime
- unknown/ambiguous state

If the current enum cannot represent unknown evidence cleanly, either:

- use `cold` plus an explicit explanation/rejected-option reason, or
- propose a small enum extension with tests.

Do not expand the enum casually. Preserve existing vocabulary unless there is a
real ambiguity the current model cannot express.

## Acceptance Criteria

- Runtime adapters do not overclaim residency.
- Explanations preserve uncertainty.
- The contract is documented and tested.
- llama-swap behavior is aligned with observed endpoint semantics.

## Validation

```powershell
cargo fmt --check
cargo test --workspace
```

