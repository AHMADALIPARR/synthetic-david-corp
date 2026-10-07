# Synthetic David Corporate Control Plane

A repository foundation for the supplied corporate COBOL design. Runtime policy owns permissions and execution. Model suggestions cannot grant authority. No real financial records, transactions, balances, or credentials are included.

**Version 0.1 is a local validation prototype, not a deployed banking system.** COBOL is the primary production target. The runnable Node harness is a reference implementation for testing its contracts and invariants. It does not call the COBOL program and cannot execute payments, post a ledger, or approve migrations.

## Run on this machine

Node 24.19 or later, Git, and SWI-Prolog are available. There are no npm dependencies.

```powershell
Set-Location C:\Users\NUCMINI\DataGripProjects\default\synthetic-david-corp
npm.cmd run check
npm.cmd test
npm.cmd run demo
& 'C:\Program Files\swipl\bin\swipl.exe' -q -s policy/policy_tests.pl -g run_tests -t halt
```

For local validation, provide your own JSON request and a local principal policy, then run:

```powershell
npm.cmd start -- path/to/request.json path/to/policy.json
npm.cmd run audit
```

The shipped `config/principals.json` is empty: no permissions are granted by default. The CLI is an owner-operated local tool. `requestor` is an identity assertion by that owner, not network authentication. Do not expose it as a service. See [request contract](docs/contracts.md). SQLite evidence is created under ignored `data/`; treat it as sensitive.

## Included

| Component | Behavior | Status |
| --- | --- | --- |
| COBOL control gate and shared copybooks | Validate input, allowlisted dispatch, halt without broker, guard completion | Source; compiler unavailable locally |
| Node reference harness | Deterministic minimum set, exact journal checks, exact migration comparison, document inspection | Runnable and tested |
| SQLite audit/provenance store | Atomic commit, replay checks, graph freezes, restart chain verification | Runnable and tested |
| Prolog policy | Minimum permitted capability cover; integer balance and exact term checks | Runnable policy tests |
| DB2 ledger DDL | Accounts, journals, entries, provenance, audit | Target schema; not executed on DB2 |
| Rails payment boundary | Explicitly raises when called | Integration source; Rails unavailable |
| Haskell types | Work items, execution state, typed roles/results | Design source; GHC unavailable |
| Payment/bank/GPU/JVM adapters | No dispatch granted | Unimplemented and disabled |
| Codex Rust harness + Qwen Responses adapter | Local text/tool wire translation through :1235 | Compiled Rust adapter; native CLI smoke tested |

The implemented agents are LEDGER, MIG-VALID, and DOCUMENT. The broader DAVID role catalog is a roadmap, not a working population of autonomous agents. The local AI stack in the adjacent `claude-backend` repository is not required or modified.

## COBOL build

With a separately installed GnuCOBOL compiler:

```text
cobc -std=cobol85 -fixed -I cobol/copybooks -x -o build/smoke cobol/smoke.cbl cobol/synthetic-david-corp.cbl
build/smoke
```

Create `build/` first. This exercises invalid identity and missing-broker rejection. No broker implementation is linked. IBM COBOL II acceptance and binary interoperability must be verified on the target compiler. Do not treat the original JVM sketch, procedure-pointer declarations, or UUID function as verified COBOL-85 features. The ABI must specify compiler, code page, numeric storage and calling convention; copybook names alone do not establish interoperability.

See [architecture](docs/architecture.md), [design corrections](docs/design-review.md), and [delivery roadmap](docs/roadmap.md). The project is hosted privately; no license for the original David code has been selected. The Codex submodule retains its upstream Apache-2.0 license and notices.
See [Codex/Qwen integration](docs/codex-qwen.md) for the upstream Rust harness, local endpoint, launch commands and transport limits.
