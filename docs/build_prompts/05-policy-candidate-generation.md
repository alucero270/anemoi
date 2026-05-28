# Prompt 05: Policy Candidate Generation

## Goal

Separate candidate generation from scoring so scheduling behavior is easier to
test and reason about.

## Required Tests

Add failing tests first.

Required test names:

- `generates_candidates_for_domain_rosters`
- `candidate_includes_residency_group`
- `candidate_includes_model_profile`
- `candidate_includes_available_supported_runtime`
- `rejects_model_without_available_runtime`
- `rejects_group_model_missing_profile`
- `candidate_order_is_deterministic`

## Implementation

Work in `anemoi-policy`.

Introduce an internal or public candidate generation function if useful.

Candidate data should include:

- action candidate
- model id
- runtime id
- residency group id
- residency state
- load estimate
- rejection reasons, when applicable

Do not change scoring behavior in this prompt except where required to expose
candidate generation cleanly.

## Acceptance Criteria

- Candidate generation is independently testable.
- Rejected options are preserved for explanations.
- Candidate order is deterministic before scoring.
- Existing continuity test still passes.

## Validation

```powershell
cargo fmt --check
cargo test -p anemoi-policy
```

