# Anemoi Execution Rules

---

## 1. What Anemoi Is

Anemoi is a local-first inference governance layer for heterogeneous AI systems.

Anemoi provides:

- runtime selection
- residency governance
- continuity preservation
- execution economics
- deterministic policy evaluation
- structured decision explanations
- telemetry for scheduling decisions

Anemoi decides:

```text
What should execute?
Where should it execute?
Should execution happen now?
What resources should remain resident?
What is the cheapest acceptable path?
Why was that decision made?
```

---

## 2. What Anemoi Is Not

Anemoi is not:

- an inference runtime
- a model host
- a LiteLLM clone
- an OpenRouter clone
- a provider gateway in core v1
- an agent framework
- a memory system
- a RAG platform
- a vector database
- a training system
- a tool orchestrator

Runtimes execute. Anemoi decides.

---

## 3. Current Repository State

Repository evidence as of 2026-05-24:

- The active Rust workspace is defined in `Cargo.toml`.
- Rust crates live under `crates/anemoi-*`.
- The example Anemoi policy config is `config/anemoi.example.yaml`.
- `src/Anemoi.*` and `Anemoi.sln` are legacy C#/.NET project files still
  present in this checkout.
- Treat the legacy .NET surface as `Needs validation` unless a task explicitly
  scopes migration, deletion, or compatibility work.
- Do not delete or rename legacy files as incidental cleanup.

---

## 4. Locked Product Boundary

Anemoi owns:

- request-to-domain-to-roster-to-residency-group scheduling
- model residency state normalization
- runtime inspection through adapters
- policy scoring
- continuity fallback
- background staging decisions
- structured explanations
- decision telemetry

Anemoi does not own:

- model execution internals
- model weights
- prompt planning
- agent memory
- retrieval
- training
- provider account management
- live infrastructure mutation

---

## 5. Target Rust Crate Boundaries

```text
anemoi-core        # domain types, config, residency states, decisions
anemoi-runtime     # runtime adapter trait and runtime inspection adapters
anemoi-policy      # deterministic scheduling, scoring, continuity behavior
anemoi-telemetry   # decision logs and runtime/event telemetry
anemoi-daemon      # axum local control-plane API
anemoi-cli         # operator commands
anemoi-mcp         # minimum MCP control-plane adapter
```

Rules:

- Domain crates should not perform network I/O.
- Runtime-specific protocol details belong in `anemoi-runtime`.
- Policy scoring belongs in `anemoi-policy`.
- Telemetry persistence belongs in `anemoi-telemetry`.
- API and CLI surfaces should orchestrate existing services, not duplicate
  policy logic.
- MCP tools should adapt to existing services, not duplicate scheduler,
  runtime, or telemetry logic.
- Do not introduce provider-specific payloads into `anemoi-core`.

---

## 6. Scheduling Model

Scheduling target is not:

```text
request -> model
```

Scheduling target is:

```text
request -> domain -> roster -> residency group -> profile -> runtime
```

Every decision must produce an explanation with reasons and rejected options
where relevant.

---

## 7. Residency States

Use the established residency vocabulary:

- `cold`
- `loading`
- `warm_cpu`
- `partial`
- `hot_gpu`
- `serving`
- `draining`
- `evicting`
- `failed`

Do not invent alternate state names unless the existing model cannot represent
a real, reviewed runtime observation.

---

## 8. Continuity Policy

Anemoi should prefer responsive degraded execution over unexplained blank waits
when policy allows.

The first proof behavior is:

```text
large model would cold-load
small worker is already hot
interactive latency budget is tight
=> select hot worker now
=> stage large model in background when allowed
=> explain the decision
```

Do not mark continuity behavior as complete unless tests validate the decision
action, selected model, staged model, and explanation reason.

---

## 9. Runtime Adapter Rules

Adapters may inspect or hand off to runtimes. They must not become policy
engines.

Initial adapter priority:

1. `MockRuntimeAdapter`
2. `LlamaSwapAdapter`
3. `OllamaAdapter`
4. `LlamaCppAdapter`

The mock adapter is the default for deterministic tests.

---

## 10. Configuration Rules

- Keep config explicit and reviewable.
- Prefer YAML for local policy configuration.
- Do not hardcode private hostnames, tokens, or environment-specific paths in
  production code.
- Treat cloud execution as future, policy-controlled, and opt-in.
- Do not commit secrets.

---

## 11. Testing Policy

Tests should prioritize policy behavior before runtime integration.

Required early coverage:

- config parsing
- candidate generation
- hot resident reuse
- cold-load avoidance
- continuity background staging
- runtime unavailable behavior
- memory pressure scoring
- explanation completeness
- decision log persistence
- API/CLI smoke behavior where relevant

Core policy tests should be single-purpose: one rule, one behavior, one reason
to fail.

---

## 12. Validation Expectations

For Rust changes, run the strongest applicable checks:

```powershell
cargo fmt --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

If Rust tooling is unavailable, report that explicitly.

Do not run live runtime or infrastructure commands unless the user explicitly
asks for live validation in the current task.

---

## 13. Live Runtime Safety

Live runtime work is allowed only when explicitly requested.

Before live runtime changes:

- confirm the runtime owner
- confirm host path and config path
- capture read-only current state
- create rollback snapshots before editing live config
- make one change at a time
- validate before continuing
- record remaining `Needs validation` items

Never move secrets, model credentials, private prompts, transcripts, generated
user data, or host-only sensitive values into Git.

---

## 14. Reasoning Context

Each task must build understanding from:

- `AGENTS.md`
- `README.md`
- `CONTRIBUTING.md`
- `Cargo.toml`
- `config/anemoi.example.yaml`
- relevant crate source
- current working tree state
- legacy `.NET` files when the task touches migration or cleanup

Use repository evidence over assumptions. Use `TBD`, `Unknown`, or `Needs
validation` instead of guessing.

---

## 15. Anti-Patterns

Do not:

- turn Anemoi into an inference runtime
- turn Anemoi into a provider gateway before core residency policy works
- hide scheduling reasons
- make policy scoring probabilistic in core v1
- duplicate scheduling logic across API, CLI, and policy crates
- erase legacy files without an explicit migration task
- weaken local-first or security constraints
- claim a runtime adapter is complete because a mock test passed

---

## 16. Prompt Completion Discipline

### Promotion Rule

A prompt is not complete until all of the following are true:

- The procedure or design document exists.
- The required test names declared in `docs/build_prompts/` exist by exact
  name in the crate test output.
- `cargo test --workspace` passes with those tests present.
- Any safety gate, opt-in flag, or permission boundary described in the
  prompt is enforced by code, not only by documentation.
- `docs/test_roadmap.md` status is updated to `Passing` in the same commit.
- The work is committed. Uncommitted work does not exist.

Documentation alone does not close a prompt.

### No Skipped Prompts

Do not begin prompt N+1 while prompt N is incomplete. Do not leave gaps in
the prompt sequence. If a prompt is deferred, mark it `Deferred` in the
roadmap with a reason.

### Required Test Names Are Declared Upfront

Each build prompt in `docs/build_prompts/` must list exact test function
names under `## Required Tests` before implementation begins. Those exact
names must appear in `cargo test --workspace` output to pass. Renaming or
omitting a required test name is a promotion failure.

### Commit Cadence

Each prompt lands as one or more commits. Do not start a new prompt while
a prior prompt has uncommitted scaffolding. Use `git status` to confirm
before starting.

### Roadmap Is Source of Truth

`docs/test_roadmap.md` is the canonical record of what is done and what is
pending. Updating its status is part of the prompt's commit — not a
separate follow-up. A prompt whose tests pass but whose roadmap still says
`Pending` is not promoted.

### Verification Before Claiming Done

Before marking a prompt `Passing`, run:

```powershell
cargo fmt --check
cargo test --workspace
```

Confirm the prompt's required test names appear by name in the output.
`cargo test` passing is necessary but not sufficient — the named tests must
be present.

### Safety Gates Must Be Code-Enforced

If a prompt describes a permission boundary, opt-in flag, or approval gate
(example: `ANEMOI_ENABLE_LIVE_EXECUTE=1`), that gate must be enforced in
daemon or CLI code. A gate that exists only in documentation provides no
protection.

### Branch Discipline

Work on branches, not main. Each branch covers one prompt or one coherent
fix. Do not commit directly to main unless merging a completed branch.

---

## Final Principle

Continuity > silence
Explanation > magic
Policy > improvisation
