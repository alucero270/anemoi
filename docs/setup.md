# Setup

## Local prerequisites

- .NET 10 SDK
- At least one supported backend:
  - Ollama
  - llama.cpp server

## Start the router

```powershell
dotnet run --project src/Anemoi.Api
```

The API listens on the ASP.NET Core default URLs unless overridden by `ASPNETCORE_URLS`.

## OpenWebUI target

Point OpenWebUI at:

```text
http://localhost:5000/v1
```

Adjust the host and port to match your local router configuration.

## Docker

```powershell
docker compose -f deploy/docker/docker-compose.yml up --build
```

The compose file mounts `deploy/config/appsettings.example.json` into the container as `appsettings.json`.
