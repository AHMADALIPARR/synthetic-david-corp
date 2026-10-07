# Synthetic David Corporate Control Plane

A repository foundation for the supplied corporate COBOL design. Runtime policy owns permissions and execution. Model suggestions cannot grant authority. No real financial records, transactions, balances, or credentials are included.

**Version 0.2 is a local validation prototype, not a deployed banking system.** COBOL is the primary production target. Rust implements the runnable validation harness, persistent audit store, CLI and Qwen adapter. This increment adds read-only COBOL structural inspection with a versioned AnalysisIR and rechecks current principal permissions before serving cached results. REXX supplies fixed-action build/launch glue. The harness does not call the COBOL program and cannot execute payments, post a ledger, or approve migrations. No project-owned JavaScript, npm package or PowerShell helper remains.

## Run on this machine

Execution is subject to the [SnapKitty Sovereign Commercial License](SNAPKITTY%20SOVEREIGN%20COMMERCIAL%20LICENSE). An embedded Rust gate requires an issuer-signed, paid, deployment-bound entitlement sealed with AES-256-GCM. The public signing root is deliberately unconfigured; protected commands refuse execution until authority and a genuine entitlement are provisioned. See [provisioning and limits](docs/execution-entitlements.md). This controls license-fee access; it does not execute bank payments.

Rust, Git, SWI-Prolog and a local Regina REXX interpreter are available. Tool downloads are excluded from Git. Run from the repository root:

```powershell
Set-Location C:\Users\NUCMINI\DataGripProjects\default\synthetic-david-corp
& .tools/regina/rexx.exe rexx/david.rexx build
& .tools/regina/rexx.exe rexx/david.rexx test
& .tools/regina/rexx.exe rexx/david.rexx license-status
& .tools/regina/rexx.exe rexx/david.rexx demo
& 'C:\Program Files\swipl\bin\swipl.exe' -q -s policy/policy_tests.pl -g run_tests -t halt
```

For local validation, provide your own JSON request and a local principal policy, then run:

```powershell
& .\rust\target\debug\david.exe run path/to/request.json path/to/policy.json
& .\rust\target\debug\david.exe audit
```

The shipped `config/principals.json` is empty: no permissions are granted by default. The CLI is an owner-operated local tool. `requestor` is an identity assertion by that owner, not network authentication. Do not expose it as a service. See [request contract](docs/contracts.md). SQLite evidence is created under ignored `data/`; treat it as sensitive.

`demo` and `run` require a valid paid execution entitlement as well as principal policy. Missing licensing authority is an expected denial in a fresh checkout. Build and tests require no entitlement; settlement test records are labeled development fixtures.

## Included

| Component | Behavior | Status |
| --- | --- | --- |
| COBOL control gate and native Rust broker | SDABI001, licensed validation, atomic evidence commit, fail closed | Linux GnuCOBOL integration in CI; compiler unavailable locally |
| Rust validation harness | Deterministic minimum set, exact journal checks, exact migration comparison, document and COBOL inspection | Tested; licensed execution |
| COBOL AnalysisIR inspector | Explicit layout, raw source hash, declaration candidates and source spans | Runnable Rust structural inspector; no compiler or semantic proof |
| Rust entitlement gate | Ed25519 paid claims, AES-256-GCM, deployment binding, Windows DPAPI | Tested; production issuer unconfigured |
| Cloudflare FinOps + MiMo GGUF | Strict DSML read invocation, parallel analytics, optional billing evidence, audited advisory proposals | Rust harness tested; live token and trained MiMo model unconfigured |
| REXX launcher | Fixed build/test/run actions; arbitrary commands rejected | Tested on Windows and in CI |
| SQLite audit/provenance store | Atomic commit, replay checks, graph freezes, restart chain verification | Runnable and tested |
| Prolog policy | Minimum permitted capability cover; integer balance and exact term checks | Runnable policy tests |
| DB2 ledger DDL | Accounts, journals, entries, provenance, audit | Target schema; not executed on DB2 |
| Rails payment boundary | Explicitly raises when called | Integration source; Rails unavailable |
| Haskell types | Work items, execution state, typed roles/results | Design source; GHC unavailable |
| Payment/bank/GPU/JVM adapters | No dispatch granted | Unimplemented and disabled |
| Codex Rust harness + Qwen Responses adapter | Local text/tool wire translation through :1235 | Compiled Rust adapter; native CLI smoke tested |

The implemented agents are LEDGER, MIG-VALID, DOCUMENT, and COBOL-ANALYZER. The broader DAVID role catalog is a roadmap, not a working population of autonomous agents. The local AI stack in the adjacent `claude-backend` repository is not required or modified.

With a genuine paid entitlement provisioned, inspect the labeled development source fixture using `david run examples/cobol-inspect.json examples/cobol-principal.json`. The example principal is opt-in; the shipped production policy remains empty. Repeated identical requests are idempotent. See [AnalysisIR contract and limits](docs/analysis-ir.md). `david version` (or REXX action `version`) reports `0.2.0`.

On another machine, install Rust and Regina, then use `rexx rexx/david.rexx build` and `rexx rexx/david.rexx test`, or Cargo directly. No Node runtime is needed. The pinned upstream Codex submodule remains unchanged; its optional SDK/web examples can contain JavaScript, but this integration launches the native Rust executable directly. MCP transport can also be implemented in Rust; a corporate MCP service is not implemented in this version.

## COBOL build

With a separately installed GnuCOBOL compiler:

```text
cobc -std=cobol85 -fixed -I cobol/copybooks -x -o build/smoke cobol/smoke.cbl cobol/synthetic-david-corp.cbl
build/smoke
```

Create `build/` first. The smoke program prints a single PASS marker only after invalid identity and missing-broker rejection both succeed; CI verifies that marker. Strict COBOL-85 returns status through the result area and uses EXIT PROGRAM, without a process RETURN-CODE register. This missing-broker smoke deliberately links no broker implementation. For the actual Linux GnuCOBOL-to-Rust runtime, build with `david broker-build` and invoke `david cobol-run REQUEST POLICY DB`; see [native broker](docs/cobol-broker.md) for licensing, the byte ABI and integration checks. IBM COBOL II acceptance and binary interoperability must be verified on the target compiler. Do not treat the original JVM sketch, procedure-pointer declarations, or UUID function as verified COBOL-85 features. The ABI must specify compiler, code page, numeric storage and calling convention; copybook names alone do not establish interoperability.

See [architecture](docs/architecture.md), [design corrections](docs/design-review.md), and [delivery roadmap](docs/roadmap.md). The project is hosted privately under the supplied SnapKitty license. The Codex submodule retains its upstream Apache-2.0 license and notices.
See [Codex/Qwen integration](docs/codex-qwen.md) for the upstream Rust harness, local endpoint, launch commands and transport limits.
See [Cloudflare FinOps](docs/cloudflare-finops.md) for the bespoke MiMo GGUF agent, DSML invocation contract, configuration and metric limits.
