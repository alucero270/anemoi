# Prompt 13: MCP Minimum Surface

## Goal

Add the minimum MCP surface after the core daemon and policy loop are stable.

## Required Tests

Add failing tests first.

Required test names:

- `mcp_lists_expected_tools`
- `mcp_decide_returns_same_decision_shape_as_http_api`
- `mcp_status_returns_runtime_and_policy_summary`
- `mcp_residents_returns_normalized_snapshots`
- `mcp_explain_returns_recorded_explanation`
- `mcp_rejects_invalid_decide_request`

## Implementation

Add an MCP crate only when needed, likely:

```text
crates/anemoi-mcp
```

Minimum tools/resources:

- `get_status`
- `list_residents`
- `decide`
- `explain_decision`
- `check_policy`

MCP should adapt protocol calls to existing core services. It must not duplicate
scheduler, runtime, or telemetry logic.

## Acceptance Criteria

- MCP and HTTP decision response shapes stay semantically equivalent.
- MCP does not require a database.
- MCP can run locally against the same config and runtime adapters.
- Invalid requests return clear diagnostics.

## Validation

```powershell
cargo fmt --check
cargo test --workspace
```

