# Anemoi

Stage 1 scaffolding for a local AI router that presents a stable OpenAI-compatible endpoint to OpenWebUI while routing requests to local inference backends.

## Solution Layout

- `Anemoi.sln`
- `src/Anemoi.Api`: ASP.NET Core API, controllers, middleware, startup
- `src/Anemoi.Core`: canonical router models, routing, fallback, health, configuration
- `src/Anemoi.Backends.Ollama`: Ollama adapter
- `src/Anemoi.Backends.LlamaCpp`: llama.cpp adapter
- `src/Anemoi.Tests`: unit, adapter, and integration tests
- `deploy/docker`: container assets
- `deploy/config`: example configuration
- `docs`: setup notes

## Stage 1 Features

- OpenAI-style `POST /v1/chat/completions` with streaming and non-streaming support
- `GET /v1/models` backed by configured UI-visible aliases
- `GET /health` and `GET /health/backends`
- Deterministic alias selection: explicit alias, keyword rule, then default alias
- Typed configuration with startup validation
- Serilog console and rolling file logging
- Ollama and llama.cpp backend adapters behind canonical interfaces

## Build And Test

```powershell
dotnet restore Anemoi.sln
dotnet build Anemoi.sln
dotnet test Anemoi.sln
```

## Run Locally

```powershell
dotnet run --project src/Anemoi.Api
```

The router loads configuration from `src/Anemoi.Api/appsettings.json`. A deployment-oriented example config is in `deploy/config/appsettings.example.json`.
