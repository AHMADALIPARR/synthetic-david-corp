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

| Type | Payload fields beyond source | Result |
| --- | --- | --- |
| LEDGER-VALIDATE | journalId UUID, currency three uppercase letters, entries list | Exact per-journal balance; never posts |
| MIGRATION-VALIDATE | modernSource, legacy, modern, equivalence `EXACT-CANONICAL-JSON-V1` | Exact comparison; legacy stays source of truth |
| DOCUMENT-INSPECT | text | Hash checked; characters and lines counted |

Entries contain `entryId`, `accountId` UUIDs, `debit`, and `credit`. Exactly one side must be positive. At least two entries, at most 256. Entry IDs cannot repeat. Currency is a single journal label; ISO membership and account currency must be verified by a future trusted ledger adapter. No tolerance is used for balance.

Provenance results carry evidence, sources, transforms, rule chain, parent IDs, assumptions, agent/tool IDs, input/output hashes, timestamp, uncertainty and risk. Uncertainty zero means deterministic transformation, not verified source truth. Source authenticity is explicitly unverified. Ledger and migration inputs are supplied snapshots, not authoritative database reads. Exact comparison is not proof of program equivalence across all inputs.

Responses are COMPLETED, HALTED, or ESCALATED. A mismatch escalates and freezes the graph. Any other admitted failure also freezes it. No automated thaw API exists. Replaying an identical completed request returns stored evidence without executing again. Reusing an ID for a different request fails. Invalid identities, oversized payloads and detected credentials are rejected before admission; their raw content is not persisted.

The COBOL copybooks define a separate native ABI, including the full 2048-byte evidence field and error fields. A broker must translate JSON and typed native representations, verify all hashes and source references, enforce the full contract, and commit before returning OK. It must preserve risk/evidence fields across commit. No cross-language bridge is implemented yet.
