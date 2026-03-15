# Contributing to Anemoi

Anemoi follows strict commit conventions and disciplined change control to keep the
project readable, maintainable, and portfolio-grade.

Codex may assist with implementation, but all changes must follow these conventions.

---

# Git Commit Guidelines

Each commit message consists of:
- Header (required)
- Body (strongly encouraged)
- Footer (optional)

No line may exceed 100 characters.

## Commit Message Format

type(scope): subject

Body (optional but strongly encouraged)

Footer (optional)

The header is mandatory.
The scope is required for this project.

## Revert

If reverting a commit:

revert: <original header>

Body must include:
This reverts commit <hash>.

## Type

Must be one of:
* feat:     A new feature
* fix:      A bug fix
* refactor: Code change without feature or bug fix
* perf:     Performance improvement
* test:     Add or update tests
* docs:     Documentation-only changes
* style:    Formatting only (no logic changes)
* chore:    Tooling, build, or dependency changes
* ci:       CI/CD workflow changes

## Scope

Scope describes the subsystem affected.

Approved scopes for Anemoi:
* core       Canonical models, interfaces, orchestration primitives
* routing    Alias, profile, rule, and route decision logic
* api        ASP.NET Core controllers, middleware, northbound API behavior
* backends   Shared backend integration work spanning multiple adapters
* ollama     Ollama adapter implementation
* llamacpp   llama.cpp adapter implementation
* config     Configuration binding, validation, options, appsettings
* health     Health reporting and diagnostics endpoints
* models     `/v1/models` behavior and model metadata exposure
* logging    Structured logging, trace context, observability plumbing
* tests      Automated tests and test infrastructure
* docs       Documentation updates
* repo       Repository structure changes
* ops        Docker, compose, deployment, and environment operations
* ci         CI workflow changes

Use lowercase.

Examples:
* feat(api): add streaming chat completion controller
* feat(routing): add capability-aware route decision metadata
* fix(ollama): normalize done_reason in streaming adapter
* chore(ops): add compose override for private ollama host

## Subject Rules

The subject must:
- Use imperative present tense ("add", not "added")
- Not capitalize the first letter
- Not end with a period
- Be concise and descriptive

Correct:
+ add streaming fallback guard
+ enforce startup config validation

Incorrect:
+ Added startup validation.

## Body

Use imperative tense.
Explain:
- What changed
- Why it changed
- How it differs from previous behavior

Keep lines under 100 characters.

Example:
Add request-scoped route logging to chat execution.

Previously the router logged success and failure without consistently attaching the
selected alias, backend, or request identity. Add a scoped log context so diagnostics
can reconstruct the execution path.

Closes #12

## Footer

Used for:
- Referencing issues: "Closes #12", "Refs #14"
- Breaking changes: must begin with "BREAKING CHANGE:"

---

# Branching Strategy

- One branch per issue
- Branch name format:
  issue/<number>-short-description

Examples:
issue/3-ollama-live-validation
issue/7-role-routing-foundation
issue/12-streaming-fix

Rules:
- Merge via Pull Request
- Squash only if commits are noisy
- Do not push directly to main

---

# Code Style

## .NET / C#

- Target `.NET 10`
- Nullable reference types stay enabled
- Use `async` end-to-end for I/O paths
- Accept and propagate `CancellationToken` on backend and API flows
- Use controllers, not Minimal APIs, unless the project direction changes explicitly
- Use `System.Text.Json`
- Use strongly typed options for router configuration
- Use `IHttpClientFactory` with typed clients for outbound backend calls
- Use `ILogger<T>` in application code and Serilog as the configured provider

## Architecture Rules

- Keep canonical router models in `Anemoi.Core`
- Keep backend-specific DTOs inside adapter projects only
- Do not leak Ollama or llama.cpp payload types into `Anemoi.Api` or `Anemoi.Core`
- Keep `Program.cs` focused on host setup, DI, middleware, controllers, and health
- Prefer explicit route decisions and structured state over hidden conversational logic
- Avoid speculative abstractions for later phases until the current phase is stable

## Testing

- Use `xUnit` for automated tests
- Use `Moq` where mocking is the simplest option
- Add or update tests for routing, adapter behavior, and API behavior when relevant
- Run:
  - `dotnet build Anemoi.sln`
  - `dotnet test Anemoi.sln`

## Naming

- Namespaces: `Anemoi.*`
- Classes, records, enums: PascalCase
- Methods, locals, parameters, fields: camelCase
- Constants: PascalCase or UPPER_SNAKE_CASE only when clearly appropriate
- Config sections and JSON fields should remain stable and explicit

## Logging

- Use structured logs only
- Include request identity and routing context where practical
- Do not log secrets, tokens, or private prompts unnecessarily
- Avoid logging raw transcripts by default unless debugging specifically requires it

## Config

- Runtime config is JSON-based (`appsettings.json`, environment overrides, or mounted files)
- Do not hardcode environment-specific paths or private host addresses in production code
- Prefer environment variables or deployment-specific config for secrets and host overrides
- Validate critical router configuration at startup

---

# Security Basics

- No secrets in git
- Use environment variables or deployment config for credentials and tokens
- Keep strict timeouts on outbound HTTP calls to local or remote backends
- Treat future cloud execution as policy-controlled and opt-in, not default
- Keep private or sensitive work local unless policy explicitly allows otherwise

---

# Pull Requests

- Describe the user-visible or operator-visible impact
- Note routing, backend, or configuration changes explicitly
- Reference the issue and milestone being advanced
- Include test coverage changes when relevant
- Call out any intentionally deferred work or known gaps

---

# Example Commits

feat(api): add openai-compatible streaming chat endpoint

Add controller support for streamed chat completion responses.
Normalize router stream events into OpenAI-style SSE chunks.

Closes #5

fix(ollama): handle malformed upstream response payloads

Previously malformed Ollama JSON could surface as an unclassified failure.
Map the error to an upstream protocol exception for clearer diagnostics.

Closes #8

chore(ops): add docker compose starter config

Add a simple compose file and example appsettings mount for local deployment.
Document the expected host mapping for backend services.

Refs #3
