# Prompt 20: Controlled Execution Gate

## Goal

Define the approval gate for any future live execution validation involving
model load, unload, staging, or inference handoff.

## Scope

Planning and safety gate only. Do not perform live mutating actions in this
prompt.

## Required Decision

Before any live mutating validation, a maintainer must approve:

- runtime target
- exact endpoint or command
- model id
- expected load/unload/execution behavior
- timeout
- rollback or recovery plan
- observation method
- success criteria
- stop conditions

## Required Tests

If code adds a guarded execution mode, add failing tests first:

- `live_execute_requires_explicit_enable_flag`
- `live_execute_rejects_without_runtime_target`
- `live_execute_returns_handoff_metadata_without_forwarding_by_default`
- `live_execute_records_mutating_action_in_decision_explanation`

## Implementation

Document the controlled execution protocol.

If code changes are made, add an explicit opt-in guard such as:

```text
ANEMOI_ENABLE_LIVE_EXECUTE=1
```

The default must remain safe:

```text
/decide allowed
/execute against mock allowed
/execute against live runtime disabled unless explicitly enabled
```

## Acceptance Criteria

- There is no accidental live model load/unload path.
- Any mutating action requires explicit opt-in.
- The operator can see exactly what will happen before it happens.
- Decision explanations record live mutating action attempts.

## Validation

```powershell
cargo fmt --check
cargo test --workspace
```

