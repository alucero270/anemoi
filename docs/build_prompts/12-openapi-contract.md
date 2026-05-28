# Prompt 12: OpenAPI Contract

## Goal

Publish and test an OpenAPI contract for the local daemon API after the API
shape is stable.

## Required Tests

Add failing tests first.

Required test names:

- `openapi_document_is_served`
- `openapi_document_includes_health_status_residents_decide_execute`
- `openapi_decide_schema_matches_decision_response`
- `openapi_explain_schema_matches_explanation_response`
- `openapi_contract_serializes_without_panic`

## Implementation

Add OpenAPI generation or a checked-in OpenAPI document, whichever best matches
the repo's Rust stack.

Document:

- request schema for `/decide`
- response schema for `Decision`
- response schema for `Explanation`
- error response shape
- explicit `/execute` limitation for v1

Do not introduce separate business logic for OpenAPI.

## Acceptance Criteria

- OpenAPI reflects the implemented daemon API.
- Contract tests fail if routes or response shapes drift.
- No provider-specific or runtime-specific DTO leaks into the API contract.

## Validation

```powershell
cargo fmt --check
cargo test --workspace
```

