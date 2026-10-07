# Architecture and authority

```mermaid
flowchart LR
    P[Caller or model proposal] --> A[Owner-managed principal policy]
    A --> R[Deterministic minimum capability set]
    R --> V[Typed read-only validation]
    V --> T[Atomic provenance and audit commit]
    V -->|failure or risk| F[Persisted graph freeze]
    T --> O[Decision-support response]
```

The diagram describes the working Rust validation harness. The COBOL control gate follows the same ordering but requires a native SD-BROKER. Broker responsibilities include authorization, graph state, deterministic sparse routing, source ingestion, foreign adapter dispatch, contract validation, risk aggregation, and atomic evidence/audit writes. The COBOL source halts without this boundary. The Rust harness is explicitly a separate validation implementation.

Project-owned execution uses COBOL, Rust and Prolog. REXX is the sole script glue and accepts fixed actions only. Codex is launched as a native executable; the Qwen adapter and launcher contain no JavaScript runtime dependency. Rails and Haskell files remain optional integration/design sources from the original language family.

Only three read-only adapters exist. There is no shell execution, HTTP endpoint dispatch, GPU embedding requirement, bank credentials, or LLM dependency. Future vector/model candidates are hints intersected with trusted capabilities and grants. The registry is immutable in-process; unknown agents are rejected, not dropped.

Audit events chain over canonical event bytes including previous hash. They bind provenance content by hash. SQL triggers prevent application-level updates/deletes. Transaction rollback prevents evidence without audit. Startup and each request verify the stored chain, and restart continues from its head.

This local store is tamper-evident within its stated threat model, not immutable storage against a machine owner. An administrator can replace the database, remove triggers, or truncate the tail; detecting tail deletion requires an independently trusted signed anchor. Graph and principal integrity also require authenticated administrative storage in production. Raw secrets and unknown sensitive text cannot be exhaustively identified by field/string checks; ingestion must exclude secrets before reaching this boundary. No production security certification is claimed.

Human migration authority is never synthesized by these adapters. Payment execution remains absent even when validation succeeds. Human review packages and authenticated authority records are future work; freezes currently require offline investigation rather than an automated resume.
