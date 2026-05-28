# Prompt 01: Config Validation

## Goal

Add explicit validation for `AnemoiConfig` before scheduling or runtime startup.

## Required Tests

Add failing tests first. Keep each test single-scoped.

Required test names:

- `accepts_example_config`
- `rejects_domain_roster_referencing_unknown_group`
- `rejects_group_referencing_unknown_model`
- `rejects_model_referencing_unknown_runtime`
- `rejects_runtime_with_unknown_adapter`
- `rejects_empty_domain_roster`
- `rejects_empty_residency_group_models`
- `reports_all_config_diagnostics_deterministically`

## Implementation

Add a config validation API, likely in `anemoi-core`.

Suggested shape:

```rust
pub struct ConfigDiagnostic {
    pub path: String,
    pub severity: DiagnosticSeverity,
    pub message: String,
}

pub enum DiagnosticSeverity {
    Error,
    Warning,
}

pub fn validate_config(config: &AnemoiConfig) -> Vec<ConfigDiagnostic>;
```

Validation should check:

- every domain roster points to an existing residency group
- every residency group has at least one model
- every residency group model exists
- every model supported runtime exists
- every runtime adapter is known for phase one
- diagnostics are sorted deterministically

Do not start runtimes from invalid config.

## Acceptance Criteria

- Example config validates cleanly.
- Invalid references produce clear diagnostics.
- Diagnostics include stable paths such as
  `domains.coding.rosters[0]`.
- Scheduler/daemon/CLI can call validation before use.

## Validation

```powershell
cargo fmt --check
cargo test --workspace
```

