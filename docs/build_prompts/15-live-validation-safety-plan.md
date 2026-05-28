# Prompt 15: Live Validation Safety Plan

## Goal

Create the live validation plan for moving Anemoi from fixture-tested behavior
to read-only validation against a real runtime.

## Scope

This prompt is documentation-first. Do not run live runtime commands unless the
user explicitly approves live read-only validation in the current task.

Default permission boundary:

```text
read-only inspection only
no config edits
no service restarts
no model load/unload
no inference execution
no secrets committed
```

## Required Inputs

Document the inputs needed before any live validation:

- runtime target: `llama-swap`, Ollama, or llama.cpp
- runtime base URL
- auth header or API key handling, if required
- small continuity worker model id
- large staged model id
- expected domain and latency budget
- operator success criteria
- rollback expectations if later live changes are approved

Suggested initial target:

```text
runtime: llama-swap
mode: read-only
small worker: qwen3.5-9b or granite-4.1-8b
large target: qwen3.6-35b-a3b
latency budget: 1500ms
```

## Required Tests

This prompt is docs/procedure only unless code changes are needed to expose
existing read-only behavior.

If code changes are required, add focused tests first:

- `live_validation_plan_requires_runtime_target`
- `live_validation_plan_marks_mutating_actions_out_of_scope`

## Implementation

Add a procedure under `docs/live_validation/` or equivalent.

The procedure must include:

- permission boundary
- required operator inputs
- read-only commands to collect current state
- where to record observations
- stop conditions
- success criteria
- next prompt link

Do not include private secrets in the procedure. Show environment-variable
placeholders instead.

## Acceptance Criteria

- A future agent/operator can tell what is allowed before live validation.
- Mutating runtime actions are explicitly out of scope.
- Missing inputs are visible as `TBD` or `Needs validation`.
- No live infrastructure change is made.

## Validation

Run text safety checks. If code changed:

```powershell
cargo fmt --check
cargo test --workspace
```

