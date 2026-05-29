# Residency Truth Contract

## Principle

False residency claims are worse than unknown state. Anemoi must not report a
model as hot just because it is configured.

## Evidence-to-State Mapping

| Anemoi State | Required Evidence | Adapter Source |
|---|---|---|
| `cold` | No runtime evidence that the model is loaded, resident, or active. | Default for configured models not returned by any inspection endpoint. |
| `loading` | Runtime reports model is being loaded (e.g., model accept handoff acknowledged but not ready). | `MockRuntimeAdapter` on load; future adapter handoff responses. |
| `warm_cpu` | Model is resident in system RAM but not actively on GPU. | Future adapter-specific CPU/GPU split evidence. |
| `partial` | Model is partially loaded (e.g., layers on GPU, some on CPU). | Future adapter-specific layer allocation. |
| `hot_gpu` | Model is loaded in GPU memory and ready to serve inference. | `OllamaAdapter`: `/api/ps` returns the model. Future adapters: runtime-specific loaded-model endpoint. |
| `serving` | Model is actively processing one or more requests. | Inferred from `active_requests` in snapshot. |
| `draining` | Runtime indicates model is being prepared for eviction. | Future adapter-specific draining signal. |
| `evicting` | Runtime confirms model is being unloaded. | Future adapter-specific eviction signal. |
| `failed` | Runtime health check fails or returns an error state. | Any adapter returning `available: false`. |

## Non-Evidence

The following do **not** prove hot residency:

| Observation | Why It Is Not Hot Evidence |
|---|---|
| Model appears in `/v1/models` | Proves configuration, not loaded state. Many runtimes list all configured models, not just loaded ones. |
| Model file exists on disk | Proves download, not runtime residency. |
| Runtime process is running | Proves process health, not model load state. |
| Another model is running | Proves runtime is capable, not that this specific model is resident. |

## Unknown State Handling

When the runtime cannot prove a model's state:

1. The model's `ResidencyState` is set to `Cold`.
2. The scheduler generates a candidate with `action: ColdLoad` and `residency_state: Cold`.
3. The decision explanation includes a reason code indicating ambiguous or
   unproven residency evidence (e.g. `"no_runtime_evidence"`).
4. The candidate does not receive the hot-resident reuse bonus.

This ensures Anemoi is conservative: unknown means cold until proven otherwise.

## Runtime-Specific Contracts

### MockRuntimeAdapter
- Returns exactly the residents configured or loaded via `load_model`.
- State changes are deterministic and under test control.

### OllamaAdapter
- `/api/ps` returns currently running models. Running status maps to `HotGpu`.
- Non-running models are not reported. Absence from `/api/ps` implies cold.
- Health check failure maps to `available: false` with empty residents.

### LlamaSwapAdapter
- `/health` confirms the process is reachable (maps to `available`).
- `/v1/models` lists configured models but does **not** prove residency.
- `inspect()` returns empty residents. Configured models remain `Cold`.
- `inspect_models()` returns configured model IDs for reference, not residency.
- The configured model list is surfaced on the snapshot as
  `configured_models: Vec<ModelId>`. This is configuration evidence, not
  residency evidence — policy may use it for candidate enumeration and
  rejected-options reasoning, but it must not contribute a hot-reuse
  bonus and it does not change the `Cold` residency state of any model.

### HttpInspectAdapter (llama.cpp / llama_server)
- Health check confirms process is reachable.
- No model-level inspection is performed. All models are `Cold`.
