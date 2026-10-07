# Cloudflare FinOps agent with MiMo GGUF and DSML

The v0.2 Rust executable `david-cloudflare-finops` implements a bespoke read-only agent for collectivekitty.com and zone 189f4aa276a12013fdef48500f6892a2. It follows the supplied parallel-query pattern while making tool dispatch explicit. There is no JavaScript, arbitrary code evaluator or shell command in this implementation.

## DSML invoke contract

```xml
<｜DSML｜tool_calls>
<｜DSML｜invoke name="cloudflare_finops_collect">
<｜DSML｜parameter name="zone_tag" string="true">189f4aa276a12013fdef48500f6892a2</｜DSML｜parameter>
</｜DSML｜invoke>
</｜DSML｜tool_calls>
```

The parser accepts exactly one tool and parameter with the configured zone. Unknown tools (including the opaque execute tool from the sample), code parameters, additional calls, unexpected attributes, extra text, non-UTF-8 and oversized input fail closed. The accepted tool performs a fixed workflow: check the real zone/domain identity, fetch two independent GraphQL windows concurrently, optionally read the explicitly configured account's billing history, then commit a provenance report. DSML is a constrained proposal protocol, not a general XML parser or executable language. This is a local native tool; no MCP server is exposed.

`agent` asks the local MiMo server to propose this envelope, validates it, runs the read workflow and commits the evidence, then requests and audits advisory recommendations. An advisory failure preserves the committed report with an explicit UNAVAILABLE status. `invoke` accepts a saved envelope. `collect` calls the same typed workflow directly. MiMo cannot select endpoints, credentials, account IDs, window filters or write operations. JSON and DSML strings from the model never become executable code.

## Build and commands

Build through Cargo or the fixed REXX build action. From the repository root:

```text
rexx rexx/david.rexx build
rexx rexx/david.rexx finops-plan
rexx rexx/david.rexx finops-dsml
rust/target/debug/david-cloudflare-finops dsml-example
rust/target/debug/david-cloudflare-finops invoke examples/cloudflare-finops.dsml config/cloudflare-finops.json data/cloudflare-finops.sqlite
rust/target/debug/david-cloudflare-finops agent path/to/local-finops-config.json data/cloudflare-finops.sqlite
rust/target/debug/david-cloudflare-finops advise REPORT_ID path/to/local-finops-config.json data/cloudflare-finops.sqlite
rust/target/debug/david-cloudflare-finops audit data/cloudflare-finops.sqlite
```

Append .exe on Windows. Plan, example generation, hashing and audit diagnostics are offline administrative actions. Collect, invoke, agent and advise use the existing paid `corporate.validation` entitlement gate, including a recheck before commit. The shipped production issuer is still empty and denies execution.

Supply `CLOUDFLARE_API_TOKEN` only in the process environment. Use least-privilege Zone Read and Analytics Read for the selected zone, with Billing Read for the selected account only if billing is enabled. No global API key is used. Requests go solely to https://api.cloudflare.com/client/v4 using certificate-validated Rustls TLS. Redirects and proxies are disabled. Only fixed zone identity, GraphQL analytics and account billing-history reads exist; there is no payment, subscription change, DNS mutation or firewall mutation.

Production credentials are currently absent. No live Cloudflare analytics or trained MiMo inference has been claimed. The adjacent workspace's MiMo source smoke uses random initialization and is not a trained GGUF. The existing Qwen process is left running and is not substituted for MiMo.

## Configure a trained MiMo model

Serve your trained MiMo GGUF with a trusted llama.cpp-compatible server bound to 127.0.0.1 on a dedicated port. Do not reuse or stop the Qwen server. Hash the selected file:

```text
rust/target/debug/david-cloudflare-finops gguf-hash /absolute/path/to/MiMo.gguf
```

Copy the shipped config to an ignored local location such as data/finops-config.json and replace mimo with:

```json
{
  "port": 1236,
  "modelId": "YOUR_ACTUAL_LOADED_MIMO_MODEL_ID",
  "ggufPath": "/absolute/path/to/MiMo.gguf",
  "ggufSha256": "YOUR_64_CHARACTER_LOWERCASE_SHA256"
}
```

The object is the value of the config's mimo field, not the whole config. Model ID must match `/v1/models`. The client checks GGUF magic and the configured full-file hash, then uses only loopback `/v1/chat/completions`. Hashing is file identity, not verification of trained quality, architecture or provenance. The server's association between model ID and actual weight bytes remains owner-declared, not remotely attested. Full-file hashing can take time for large models.

The DSML planner asks for a single typed envelope. The advisory pass requests strict JSON recommendations with evidence references to the committed report hash; extra fields/tools or missing evidence are rejected. Server support for JSON response_format is required for advise. Recommendations are marked HUMAN_REVIEW_REQUIRED and claimsVerified=false, carry model/prompt/output hashes and assumptions, and are committed to the same audit chain before being returned. Valid JSON and evidence links do not verify a model's reasoning. No recommendation is applied automatically or represented as authoritative financial advice.

## Metric and cost semantics

The 30-day window covers exactly 30 completed UTC dates ending yesterday. Four months means four calendar months, also excluding today. Inclusive start/end filters avoid the sample's extra current-day bucket. GraphQL response errors, wrong zones, duplicate dates, out-of-window data, invalid metrics and numeric overflow are refused. A failing window is UNAVAILABLE while the independent successful window remains usable. Unreturned dates are PARTIAL_OR_NO_DATA and are never filled with invented zeros; plan-specific retention can prevent the long-range query.

Daily uniq.uniques values describe unique IPs, not verified people. They are preserved per date and never summed into period unique visitors. Requests are summed only across returned dates using checked integer arithmetic. API availability and retention depend on the dataset and plan. [Cloudflare dataset semantics](https://developers.cloudflare.com/analytics/graphql-api/features/data-sets/), [GraphQL limits](https://developers.cloudflare.com/analytics/graphql-api/limits/).

Billing is disabled by default because the sample supplied no account ID. To enable it, set billingEnabled=true and accountId to the intended 32-character account ID; it must match the zone identity response. The agent reads the first page (at most 100 records) of account billing history and explicitly reports incomplete history. Events can include adjustments and payments, and they are account-scoped: they are not automatically this zone's spend. Monetary fields are preserved as observed API data and never summed as authoritative currency amounts. Period spend, unit rates and savings remain null until a future reconciled billing adapter supplies them. Request counts alone cannot establish cost. [Billing history API and permissions](https://developers.cloudflare.com/api/resources/billing/subresources/history/methods/list/).

## Provenance and limits

Each successful API response has a source ID, fixed endpoint/operation, retrieval timestamp and response hash. The report includes assumptions, coverage, responsible agent and flags showing no changes were executed. A durable SQLite transaction stores full response evidence and the report in a hash-chained event; advisory events bind the same report and proposal hashes. Diagnostics verify the chain and source hashes on restart. Triggers prevent ordinary updates/deletes and tests cover rollback and tampering.

Keep data/ private: API responses can contain sensitive account and invoice metadata. Raw responses and API tokens are not sent to MiMo; its advisory prompt contains deterministic summaries and selected billing fields. Error output uses fixed codes rather than remote bodies or request headers. Local audit integrity and configuration are trusted administrative responsibilities; a machine owner can rewrite a whole database or truncate its tail, so independent signed anchors remain future work. The CLI's local operator identity is not network authentication. There is no scheduler, write-back, production billing reconciliation or MiMo training in this increment.
