# Anemoi for Pi Agent

Anemoi integrates with Pi via the [MCP (Model Context Protocol)](https://modelcontextprotocol.io/) adapter in `crates/anemoi-mcp`.

## Quick Start

```powershell
# Navigate to the repository
cd "C:\Users\Alex Lucero\source\repos\anemoi"

# Start the daemon on a port pi can access
cargo run -p anemoi-daemon -- --port 7071 &

# Wait for it to start, then in another terminal:
cargo run -p anemoi-mcp -- serve --port 7072
```

## Pi Integration

Anemoi exposes five MCP tools for Pi to use:

| Tool | Description |
|------|-------------|
| `get_status` | Returns daemon status, domain/model/runtime counts |
| `list_residents` | Lists current model residency states |
| `decide` | Make a scheduling decision for a request |
| `explain_decision` | Get the reasoning behind a past decision |
| `check_policy` | Validate current configuration |

### Using from Pi

```typescript
// TypeScript example
import { createClient } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';

const transport = new StdioClientTransport({
  command: 'cargo',
  args: ['run', '-p', 'anemoi-mcp', '--', 'serve'],
});

const client = await createClient({
  name: 'anemoi',
  version: '0.1.0',
  capabilities: {},
});

await client.connect(transport);

// Make a decision
const result = await client.callTool({
  name: 'decide',
  arguments: {
    domain: 'coding',
    latency_budget_ms: 1500,
  },
});
```

## Configuration

Create `config/anemoi.pi.yaml`:

```yaml
domains:
  coding:
    rosters:
      - small_swarm
      - large_models

residency_groups:
  small_swarm:
    purpose:
      - interactive coding continuity
    keep_hot: true
    allow_background_load: true
    models:
      - qwen9b
      - granite8b

  large_models:
    purpose:
      - higher quality coding synthesis
    keep_hot: false
    allow_background_load: true
    models:
      - qwen35_a3b

models:
  qwen9b:
    family: qwen
    parameter_class: 9b
    context_window: 32768
    vram_required_mb: 9000
    ram_required_mb: 12000
    cold_load_estimate_ms: 18000
    supported_runtimes:
      - mock

  granite8b:
    family: granite
    parameter_class: 8b
    context_window: 8192
    vram_required_mb: 8000
    ram_required_mb: 10000
    cold_load_estimate_ms: 15000
    supported_runtimes:
      - mock

  qwen35_a3b:
    family: qwen
    parameter_class: 35b
    context_window: 32768
    vram_required_mb: 30000
    ram_required_mb: 45000
    cold_load_estimate_ms: 45000
    supported_runtimes:
      - mock

runtimes:
  mock:
    adapter: mock
    initial_residents:
      - model_id: qwen9b
        state: hot_gpu
        vram_mb: 9000
        ram_mb: 12000

continuity:
  keep_small_worker_hot: true
  background_load: true
  max_blank_wait_ms: 1500
  prefer_degraded_response_over_silence: true
```

Then run:

```powershell
cargo run -p anemoi-daemon -- --config config/anemoi.pi.yaml
```

## Environment Variables

```powershell
# Optional: specify config location
$env:ANEMOI_CONFIG = "config/anemoi.pi.yaml"
$env:ANEMOI_BIND = "127.0.0.1:7071"
$env:ANEMOI_DECISION_LOG = "logs/anemoi-decisions.jsonl"
```

## Running in the Background

```powershell
# Start daemon in background (PowerShell 7+)
& "C:\Users\Alex Lucero\source\repos\anemoi\target\debug\anemoi-daemon.exe" --port 7071 &

# Or use nohup equivalent with Start-Process
Start-Process powershell -ArgumentList "-NoExit -Command \"cd 'C:\Users\Alex Lucero\source\repos\anemoi'; cargo run -p anemoi-daemon\""
```

## Testing

```powershell
# Validate the codebase
cargo fmt --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

## Architecture

```
┌─────────────────┐
│   Pi Agent      │
│   (MCP Client)  │
└────────┬────────┘
         │
         │ MCP Protocol
         │
┌────────▼────────┐
│  anemoi-mcp     │
│  (MCP Adapter)  │
└────────┬────────┘
         │
         │ HTTP API
         │
┌────────▼────────┐
│  anemoi-daemon  │
│   (API Server)  │
└────────┬────────┘
         │
         │ Policy Engine
         │
┌────────▼────────┐
│  anemoi-policy  │
│  (Scheduler)    │
└────────┬────────┘
         │
         │ Runtime State
         │
┌────────▼────────┐
│  anemoi-runtime │
│  (Adapters)     │
└─────────────────┘
```

## Key Features for Pi

- **Local-first**: No cloud dependencies, runs entirely locally
- **Deterministic scheduling**: Same input, same output
- **Transparent decisions**: Every decision is explained
- **No database required**: Works with in-memory state by default
- **Hot-worker reuse**: Prefers already-loaded models to reduce latency

## Next Steps

1. Build and test locally
2. Configure your runtime adapters (mock, llama-swap, ollama)
3. Enable live execution if needed (see `docs/live_validation/controlled-execution-gate.md`)
4. Integrate with your Pi agent workflow
