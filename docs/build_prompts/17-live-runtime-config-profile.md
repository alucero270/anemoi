# Prompt 17: Live Runtime Config Profile

## Goal

Add a sanitized live config profile for the first validated runtime target
without committing secrets or private host-only values.

## Scope

Config and docs only unless validation requires small config-loading code
changes.

## Required Tests

Add failing tests first.

Required test names:

- `accepts_live_llama_swap_example_config`
- `live_config_uses_environment_for_auth`
- `live_config_rejects_missing_required_runtime_url_when_no_default_exists`
- `live_config_keeps_small_worker_and_large_target_in_distinct_groups`

## Implementation

Add a sanitized example such as:

```text
config/anemoi.llama-swap.example.yaml
```

The config should include:

- `coding` domain
- `small_swarm` residency group
- `large_models` residency group
- runtime entry for `llama_swap`
- no secrets
- no private absolute host paths
- comments or docs explaining required env vars

If auth is needed, prefer config fields that reference environment variables or
document that the CLI/daemon injects headers from environment.

## Acceptance Criteria

- Example config validates cleanly.
- It does not commit secrets.
- It does not claim live residency before prompt 18 proves runtime semantics.
- It can be used for read-only inspection and decision testing.

## Validation

```powershell
cargo fmt --check
cargo test --workspace
```

