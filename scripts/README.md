# Anemoi Scripts

Scripts for running and managing Anemoi services.

## start-mcp.ps1

Starts the MCP adapter for Pi integration.

```powershell
# Default port 7072
.\start-mcp.ps1

# Custom port
$env:ANEMOI_MCP_PORT=8080; .\start-mcp.ps1
```

## Usage in Pi

From a Pi agent context, you can use the MCP client to interact with Anemoi:

```typescript
import { createClient } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';

const transport = new StdioClientTransport({
  command: 'pwsh',
  args: ['-NoProfile', '-ExecutionPolicy', 'Bypass', 
    '-Command', '.\\scripts\\start-mcp.ps1'],
});

const client = await createClient({
  name: 'anemoi',
  version: '0.1.0',
});

await client.connect(transport);
```
