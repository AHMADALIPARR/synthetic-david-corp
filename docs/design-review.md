# Corrections to the supplied draft

The supplied mixed-language design is the project brief. It is not presented as compilable or production-ready code. This repository restructures it into an executable reference harness and explicit target boundaries.

* A later initialization step must never reset HALTED to RUNNING. Gate stages execute only after prior success.
* Do not OPEN OUTPUT on an existing audit file: that can replace its contents. The reference store appends and resumes the prior head.
* Do not recurse into audit writing when audit itself fails. Persistence failure returns HALTED immediately.
* An 8192-byte input cannot be hashed through a 4096-byte buffer. Reference hashing covers the complete canonical payload.
* Results cannot be cleared before their evidence is consumed. Every result has evidence and a provenance record bound into audit.
* Every external call requires status/contract checks, including risk, response and legacy adapters. Completion follows persistence success.
* ASSEMBLER-ANALYZER exceeds the original 16-byte agent ID field. Stable wire identifiers must fit the ABI; current implemented identifiers do.
* The two supplied linkage layouts disagree on evidence length, permissions and error fields. Shared copybooks now define one native result layout.
* The prior SQL trigger body was empty. A row-level equality check cannot validate a journal assembled over multiple rows. The target schema now represents journals; the reference harness checks the whole journal. No DB2 posting enforcement is claimed.
* Financial amounts are DECIMAL(19,4), matching 15 integer digits plus four fractional digits. Balance is exact by default; the original 0.005 tolerance is removed. Any future tolerance must have an explicit named policy and decision provenance.
* UUID generation, procedure pointers, EXIT SECTION and JVM calls in the brief require target-specific validation. The gate uses a named broker CALL and ordinary paragraphs. IBM compatibility remains unverified.
* GPU vector similarity is not a proof of authorization, semantic equivalence, or source truth. No such inference is used.

The proposed Treasury, Quant, asset-class, mainframe-analysis, modernization and research roles remain scoped integration work. Placeholder success responses would violate the stated rules, so they are not enabled.
