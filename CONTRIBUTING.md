# Contributing

## Workflow

1. Create a branch from `main`.
2. Keep changes scoped to a single task when practical.
3. Run `dotnet build Anemoi.sln` and `dotnet test Anemoi.sln` before opening a pull request.

## Standards

- Target `net10.0`.
- Keep canonical models in `Anemoi.Core`.
- Keep backend-specific DTOs inside adapter projects.
- Use structured logging only.
- Prefer additive extension points over speculative abstractions.

## Pull Requests

- Describe the user-visible or operator-visible impact.
- Note configuration changes explicitly.
- Include test coverage for routing, adapters, or API behavior when relevant.
