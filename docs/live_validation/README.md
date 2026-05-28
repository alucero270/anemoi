# Live Validation

This directory contains procedures for read-only and controlled live validation
of Anemoi against real runtimes.

## Files

| File | Purpose |
|---|---|
| `safety-plan.md` | Permission boundary, operator inputs, and procedure for read-only live validation. |
| `TBD` | Read-only probe results will be recorded here after prompt 16. |
| `TBD` | Live decision smoke results will be recorded here after prompt 19. |

## Phase Policy

Live validation follows a strict phase policy:

1. **Read-only inspection** (prompts 15-19): HTTP GET probes, decision smoke
   tests, no runtime mutation.
2. **Controlled execution** (prompt 20+): Requires explicit opt-in, approval,
   and documented rollback plans before any load/unload or inference handoff.

No live runtime command may be run without explicit user approval in the current
task.
