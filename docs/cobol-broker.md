# Native COBOL broker (v0.2)

`SYNTHETIC-DAVID-CORP` calls Rust's exported `SD_BROKER` once with `EXECUTE-AND-COMMIT`. Rust checks the paid entitlement, current principal grants, typed payload and source hashes. It selects the minimum permitted agent, validates the native representation, and atomically commits provenance, audit, replay state and graph freezes before returning. Payments, posting, migration approval and arbitrary shell/network dispatch remain disabled.

The target is Linux GnuCOBOL using its native C calling convention and ASCII DISPLAY fields. IBM COBOL II, EBCDIC, packed decimal and JVM interoperability are unverified. Rust produces an rlib and cdylib; `cobc -K SD_BROKER` resolves the C entry point. Deploy matching binaries, library and copybooks together.

## Build and run

Install Rust 1.99, GnuCOBOL and optionally Regina REXX. From the repository root on Linux:

```text
rexx rexx/david.rexx build
rexx rexx/david.rexx broker-build
rust/target/debug/david cobol-run examples/cobol-inspect.json examples/cobol-principal.json data/native-audit.sqlite
rust/target/debug/david audit data/native-audit.sqlite
```

Cargo and `david broker-build` also work directly. The CLI uses fixed compiler arguments and a fixed native runner with no shell interpolation. Windows returns an unsupported-target error for broker-build and cobol-run.

Provision a genuine issuer-pinned build and deployment-bound entitlement as described in [execution entitlements](execution-entitlements.md). The shipped issuer root and principal policy remain empty, so a fresh checkout halts. Example policy grants inspection only. TEST-ONLY settlement claims used in tests are not production payment evidence.

The C entry requires `DAVID_PRINCIPALS_FILE` (owner-controlled JSON, at most 256 KiB) and `DAVID_AUDIT_DB` (durable SQLite path; :memory: refused), plus existing entitlement/key settings. `cobol-run` supplies policy/database paths and scopes library loading to the repository debug directory. This is a local owner-operated boundary: requestor is an identity assertion, not service authentication. Binaries, policy, environment, audit and graph state require trusted administrative ownership.

## SDABI001 contract

All four arguments are BY REFERENCE. Command is 64 ASCII bytes space padded. Return area is four ASCII digits: 0000 for a valid COMPLETED or ESCALATED result, 0016 for HALTED. The C function also returns integer 0 or 16; COBOL relies on the explicit return area. Pointers must be live, nonoverlapping, race-free and exactly sized. Null pointers return 16; invalid non-null pointers cannot be checked safely.

Record alignment is one. Numeric fields use DISPLAY digits without signs or native binary storage. Text is ASCII space padded. Payload is UTF-8 JSON with an eight-digit byte length and space-filled unused capacity. Oversized fields, invalid padding/numbers/version and control characters are refused. This version replaces the earlier sketch's COMP fields.

| Request field | Offset | Bytes |
| --- | ---: | ---: |
| ABI SDABI001 | 0 | 8 |
| Request / trace / graph UUID | 8 / 44 / 80 | 36 each |
| Requestor | 116 | 64 |
| Request type | 180 | 32 |
| Payload byte length | 212 | 8 |
| JSON payload | 220 | 8192 |

Total request: **8412 bytes**.

| Result field | Offset | Bytes |
| --- | ---: | ---: |
| ABI | 0 | 8 |
| Status | 8 | 16 |
| Evidence reference JSON | 24 | 2048 |
| Provenance UUID | 2072 | 36 |
| Agent / tool | 2108 / 2124 | 16 / 32 |
| Input / output SHA-256 | 2156 / 2220 | 64 each |
| Timestamp | 2284 | 27 |
| Uncertainty / risk / error | 2311 / 2315 / 2319 | 4 each |
| Error message | 2323 | 256 |

Total result: **2579 bytes**. Uncertainty uses four implicit decimal digits for PIC 9V999. Current deterministic adapters return 0000; source-truth limitations remain in provenance.

COMPLETED evidence is a SQLITE-PROVENANCE-V1 reference containing request ID, provenance ID and output hash. Full evidence, transformations, sources, assumptions and uncertainty live in the committed SQLite node. Consumers resolve it from that store and verify the audit chain and hashes. Large AnalysisIR results are not truncated into the fixed field. ESCALATED evidence is an AUDITED-REQUEST-V1 reference to the durable request response hash, with positive risk and error 0008; no successful provenance is fabricated. HALTED uses error 0016 and a specific error name. Pre-admission and license denials do not commit evidence.

The runner reads exactly one 8412-byte record from DD_NATIVEIN and writes one 2579-byte record to DD_NATIVEOUT. Empty, short or extra records fail before dispatch. cobol-run creates private temporary call files under ignored data/, removes them after decoding and exits unsuccessfully for HALTED or ESCALATED. Always inspect the result: COBOL-85 process exit alone is not validation status. Interrupted or failed calls can retain private call files for diagnosis.

## Verification

Unit tests cover layout, numeric/version errors, license refusal, evidence binding, boundary-validation failure and cache tampering. Cached responses are bound to the verified final audit event's request/response hashes.

Linux CI explicitly runs native_broker_roundtrip using real GnuCOBOL and the production Rust library. An isolated source copy pins a TEST-ONLY issuer and uses authentic signed, sealed fixture entitlements. Tests cover inspection, exact journal arithmetic, replay, revoked grants, restart, ID conflict, malformed framing, escalation/freeze, rollback and cache tampering, then rebuild against the shipped root to check license refusal. This adds no production licensing bypass and changes no tracked issuer configuration. The separate unlinked smoke proves missing broker rejection.
