# Prompt 24: Resource Pressure Model

## Goal

Add a first-class resource pressure model for VRAM, RAM, KV cache, active
requests, and estimated load cost.

The scheduler should score with explicit pressure evidence instead of opaque
penalties.

## Scope

Allowed:

- normalized pressure calculations
- pressure reasons in explanations
- configurable thresholds
- unknown pressure handling
- mock and fixture tests

Not required:

- perfect vendor-specific GPU accounting
- multi-node scheduling
- predictive ML optimization

## Required Tests

Add failing tests first:

- `pressure_model_calculates_vram_pressure_from_snapshot`
- `pressure_model_calculates_ram_pressure_from_snapshot`
- `pressure_model_preserves_unknown_when_capacity_is_missing`
- `high_pressure_penalizes_cold_load_candidate`
- `pressure_explanation_names_vram_ram_and_unknown_inputs`
- `active_request_pressure_penalizes_busy_runtime`

## Implementation

Add a small pressure model, probably in `anemoi-policy` or a focused module.

Inputs:

- runtime memory snapshot
- resident memory usage
- candidate model requirements
- active request count
- cold load estimate
- latency budget

Outputs:

- normalized pressure values
- candidate penalties
- explanation reasons

Unknown data must remain unknown. Do not convert missing capacity into zero
pressure.

## Acceptance Criteria

- Scheduler decisions can explain resource pressure in plain structured
  reasons.
- Unknown memory state does not create false confidence.
- High pressure changes candidate scoring predictably.
- Existing continuity behavior still passes.

## Validation

```powershell
cargo fmt --check
cargo test --workspace
```

