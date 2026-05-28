# Prompt 09: CLI Operator Loop

## Goal

Make the CLI the first complete operator surface for the governance loop.

## Required Tests

Add failing tests first.

Required test names:

- `cli_status_prints_configured_counts`
- `cli_policy_check_reports_valid_config`
- `cli_policy_check_reports_invalid_config_diagnostics`
- `cli_decide_prints_selected_model_and_action`
- `cli_decide_prints_explanation_reasons`
- `cli_residents_prints_runtime_snapshots`
- `cli_runtimes_prints_configured_adapters`

## Implementation

Work in `anemoi-cli`.

Commands:

```text
anemoi status
anemoi residents
anemoi decide --domain coding --latency-budget-ms 1500
anemoi explain <decision-id>
anemoi runtimes
anemoi policy check
```

`policy check` should run real config validation and print actionable
diagnostics.

Do not make the CLI duplicate scheduler logic.

## Acceptance Criteria

- CLI validates config before decision commands.
- CLI output is readable and deterministic.
- `policy check` is useful before starting the daemon.
- No database is required.

## Validation

```powershell
cargo fmt --check
cargo test -p anemoi-cli
```

