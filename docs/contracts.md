# Contracts

Every admitted request has exactly `requestId`, `traceId`, `graphId` (lowercase UUID strings), `requestor`, `type`, and `payload`. Payloads are limited to 8192 UTF-8 bytes of canonical JSON. Additional fields fail validation. Money is a nonnegative decimal string with exactly four fractional digits and up to 15 integer digits. No rounding is performed.

Owner-managed principal policy shape:

```json
{
  "local-reviewer": {
    "agents": ["LEDGER", "MIG-VALID", "DOCUMENT"],
    "permissions": ["ledger:read", "migration:read", "document:read"]
  }
}
```

This grants local analysis only. Permissions must never come from the request payload or model output. Configure this in a separate local file; the shipped policy denies everyone.

Each payload supplies `source` with `id`, `location`, and lowercase `sha256`. Source IDs and locations are references supplied by the caller; they are not fetched. Hashes for structured evidence use SHA-256 over UTF-8 canonical JSON: object keys sorted recursively, arrays ordered, no whitespace. Document hashes include JSON string quoting/escaping. Exported raw-file hashes are a different format.

The Rust port retains the existing SQLite tables, triggers, chain layout and string/ordinary-number hashes; it verifies an existing chain before writing. Object keys sort by UTF-16 and floating-point values use ECMAScript formatting for compatibility. Rust additionally preserves signed/unsigned 64-bit JSON integers exactly. Old JavaScript inputs that rounded integers above 2^53 are not equivalent inputs: compare the stored bytes before migration, and always represent financial amounts as decimal strings. A chain that fails verification is refused, never reset automatically. Document character counts retain UTF-16 units for compatibility.

| Type | Payload fields beyond source | Result |
| --- | --- | --- |
| LEDGER-VALIDATE | journalId UUID, currency three uppercase letters, entries list | Exact per-journal balance; never posts |
| MIGRATION-VALIDATE | modernSource, legacy, modern, equivalence `EXACT-CANONICAL-JSON-V1` | Exact comparison; legacy stays source of truth |
| DOCUMENT-INSPECT | text | Hash checked; characters and lines counted |
| COBOL-INSPECT | text, dialect `COBOL85-FIXED` or `COBOL85-FREE` | Raw source hash checked; AnalysisIR v1 declaration candidates and spans |

COBOL inspection requires agent `COBOL-ANALYZER`, capability `legacy.cobol.inspect`, and permission `legacy:cobol:read` in the owner-managed principal policy. Its source SHA-256 covers the complete raw UTF-8 text, including line endings, rather than canonical JSON string encoding. The input layout names are inspector conventions; free layout is not a claim of COBOL-85 or IBM COBOL II compiler support. See [AnalysisIR](analysis-ir.md).

Entries contain `entryId`, `accountId` UUIDs, `debit`, and `credit`. Exactly one side must be positive. At least two entries, at most 256. Entry IDs cannot repeat. Currency is a single journal label; ISO membership and account currency must be verified by a future trusted ledger adapter. No tolerance is used for balance.

Provenance results carry evidence, sources, transforms, rule chain, parent IDs, assumptions, agent/tool IDs, input/output hashes, timestamp, uncertainty and risk. Uncertainty zero means deterministic transformation, not verified source truth. Source authenticity is explicitly unverified. Ledger and migration inputs are supplied snapshots, not authoritative database reads. Exact comparison is not proof of program equivalence across all inputs.

Responses are COMPLETED, HALTED, or ESCALATED. A mismatch escalates and freezes the graph. Any other new admitted failure also freezes it. No automated thaw API exists. Replaying an identical completed request returns stored evidence without executing again only after current licensing and principal permissions are checked. Revoked authority denies access without overwriting cached evidence or its audit. Reusing an ID for a different request fails before principal-policy lookup. Invalid identities, oversized payloads and detected credentials are rejected before admission; their raw content is not persisted.

The COBOL copybooks define the versioned SDABI001 byte ABI. The native `SD_BROKER` calls the licensed Rust validator and checks result representation before atomic commit. Its 2048-byte evidence field carries a durable provenance reference; complete evidence remains in SQLite. Replay responses must match the verified final audit event's request and response hashes. See [native broker](cobol-broker.md) for layout, compilation, status handling and deployment limits.
