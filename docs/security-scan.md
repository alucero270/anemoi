# Security Scan Report

**Date:** 2026-05-25
**Workspace:** All 7 crates under `crates/anemoi-*`
**Validation:** `cargo test --workspace` | `cargo clippy --workspace --all-targets -- -D warnings`

---

## Summary

| Check | Status |
|---|---|
| Hardcoded secrets | ✅ None found |
| `unsafe` code | ✅ Clean |
| Test suite | ✅ 92 tests passing |
| Clippy warnings | ✅ Clean |
| TLS libraries | ✅ `rustls` + `native-tls` |

---

## Test Results

- **92 tests** across 7 crates: all pass
- `cargo clippy --workspace --all-targets -- -D warnings` passes clean
- `cargo fmt --check` passes

---

## Findings

### ✅ No Hardcoded Secrets

- Auth tokens are optional and sourced from `config/anemoi.example.yaml`
- The `"secret"` value appears only in test fixtures (`crates/anemoi-runtime/src/lib.rs:785`), not production code

### ✅ Secure Defaults

| Setting | Value |
|---|---|
| Bind address | `127.0.0.1:7070` |
| Config path | `config/anemoi.example.yaml` |
| Auth tokens | Optional, runtime-configured |
| Secrets in Git | None |
| TLS for outbound | `rustls` + `native-tls` (TLS 1.2/1.3) |

---

## Observations

| Area | Finding | Risk |
|---|---|---|
| API authentication | No auth middleware on `/decide`, `/execute`, `/residents` | Low |
| Security headers | Missing `X-Content-Type-Options`, `X-Frame-Options`, CSP | Low |
| Rate limiting | No rate limits on daemon API | Low |
| Input validation | OpenAPI schema validation present | ✅ Good |
| Deserialization | Uses `serde` with safe defaults | ✅ Good |

---

## Recommendations

1. **Add authentication middleware** if daemon is network-exposed
2. **Add security headers** via tower-http:
   - `X-Content-Type-Options: nosniff`
   - `X-Frame-Options: DENY`
   - `Cross-Origin-Embedder-Policy`
3. **Add rate limiting** for production (e.g., using `axum-extra` or custom tower middleware)
4. **Document production hardening steps** before network exposure

---

## Validation Commands

```powershell
cargo fmt --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```