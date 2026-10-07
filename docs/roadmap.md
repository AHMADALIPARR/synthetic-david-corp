# Delivery roadmap

1. **Local foundation (v0.2):** tested reference validators, policy, transactional audit/provenance, frozen graphs, native COBOL gate source, and current principal authorization on cache replay. Read-only COBOL declaration inspection now emits AnalysisIR v1 with raw-source hashes and source spans through the licensed Rust runtime.
2. **Native authority:** select IBM/GnuCOBOL target, implement SD-BROKER with explicit ABI, compare Rust validation behavior against real COBOL in CI, validate EBCDIC/packed-decimal and overflow semantics. The gate already passes strict GnuCOBOL-85 smoke checks in CI.
3. **Trusted evidence and DB2:** authenticated source ingestion, full provenance DAG, per-currency account checks, complete-journal posting procedure with locks/idempotency/reversal semantics, transaction-bound human authority, restricted DB grants, independent audit anchoring.
4. **Human review:** authenticated reviewer identities, immutable decisions tied to graph/evidence hashes, scoped freeze and explicit offline/online recovery policy. A model never approves migration.
5. **Expand read-only legacy analysis:** v0.2 supplies a bounded COBOL structural inspector, not a compiler or semantic parser. Add compiler-backed COBOL validation, continuation/preprocessing/code-page support, then PL/I, assembler, JCL and DB2 adapters. Verify compiler dialect and migration equivalence separately; declaration inventory does not establish semantic equivalence.
6. **Optional orchestration:** integrate local Prolog/model/GPU services through typed adapters. Proposals are schema checked and permissions remain runtime owned. GPU buffers are not financial evidence.
7. **Rails and payment rails:** only after native authority, trusted persistence, risk gates and human approvals exist; implement allowlisted connectors with dry-run validation before any live-money capability.
8. **Research and treasury:** licensed/approved financial data feeds, timestamped evidence, uncertainty, reproducible calculations and decision-support labeling. No fabricated prices, transactions or balances.

Each step requires executable validation and documented deployment limits. The thirty proposed DAVID roles are intended capabilities, not claims of implemented agents. JVM, Haskell and Rails are optional foreign runtimes with separate compiler and deployment requirements.
