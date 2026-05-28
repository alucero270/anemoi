---
name: Feature / Prompt task
about: Track a feature, prompt, or implementation task
title: "Prompt NN: "
labels: enhancement
assignees: ""
---

## Goal

<!--
One paragraph. What does this add or change, and why does it matter?
-->

## Scope

**Allowed:**

- 

**Not allowed / not required:**

- 

## Acceptance Criteria

- [ ] 
- [ ] 

## Test Expectations

<!--
List exact test function names. These must appear verbatim in `cargo test --workspace` output before the prompt can be promoted.
See AGENTS.md Section 16 — Required Test Names Are Declared Upfront.
-->

Exact test function names required:

- `test_name_here`

## Affected Surfaces

| Crate | Change |
|---|---|
| `anemoi-` | |

## Contract Details

<!--
Types, endpoint shapes, config fields, state machines, or other contracts this prompt introduces or modifies.
-->

## Architecture Constraints

<!--
Rules this implementation must not violate. Reference AGENTS.md sections where relevant.
-->

- Domain crates must not perform network I/O.
- Policy scoring belongs in `anemoi-policy`.
- No provider-specific payloads in `anemoi-core`.

## Validation

```powershell
cargo fmt --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

## Notes / Dependencies

<!--
- Depends on: prompt N (reason)
- Required by: prompt N (reason)
- Related issues: #N
- Build prompt doc: docs/build_prompts/NN-name.md
-->
