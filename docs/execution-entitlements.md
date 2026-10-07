# Paid execution and deployment gate

The [SnapKitty license](../SNAPKITTY%20SOVEREIGN%20COMMERCIAL%20LICENSE) is implemented by `rust/execution-gate`, linked directly into the corporate runtime and Qwen endpoint. REXX only launches fixed native commands.

Protected features are `corporate.validation`, `qwen.responses`, and `codex.launch`. Corporate checks occur before replay/cache lookup and again before completing analysis. Qwen checks every inference request. Native Codex launch checks its feature before spawning. Missing entitlement or unknown signing authority means refusal. Audit inspection, deployment identification, license diagnostics, compilation and tests remain available for administration. The separate COBOL gate still halts without SD-BROKER; any future broker must invoke this verifier before returning authorization. Upstream Codex and LM Studio remain independent third-party programs.

## Cryptographic contract

The licensor signs a strict typed entitlement with Ed25519, using domain-separated deterministic JSON bytes. Claims bind the product, licensee, feature set, deployment fingerprint, validity interval, and settlement evidence reference/hash. Only `SETTLED` grants covering the entire licensed interval are accepted. Pending, unpaid, refunded, revoked, missing or out-of-period settlement claims fail closed. The licensor must independently verify actual payment before signing; this repository has no payment processor integration and does not collect money.

The customer verifies the signature against the public key compiled from `config/licensing-root.hex`, then encrypts the signed grant locally with AES-256-GCM. Encryption uses a random 96-bit nonce and authenticates the product/domain/deployment as associated data. Decryption authenticates the tag and verifies the issuer signature again. Holding the AES key does not permit forging an issuer signature.

The runtime derives its deployment fingerprint from Windows MachineGuid or Linux `/etc/machine-id`, never a caller-supplied machine ID. Windows wraps the local 256-bit AES key with current-user DPAPI. Linux requires a private key file with no group/other permissions. Other operating systems currently fail closed. Issuer private keys are never embedded. Environment variables select files, not a replacement trusted issuer or a `paid=true` override. Claims reject unknown fields and unsupported algorithms. Raw keys and decrypted buffers use zeroizing containers; Windows DPAPI output is erased before freeing. Successful corporate responses bind entitlement ID, issuer fingerprint and licensed interval into the response/audit hash without storing private keys or receipt contents.

## Provisioning

The repository deliberately ships an empty public trust root. It creates no real payment or production entitlement. Protected operations return `LICENSE_ISSUER_UNCONFIGURED` until provisioned. Tests use labeled disposable fixtures through test-only access to the deterministic core; release builds have no skip-license switch.

Build with `rexx rexx/david.rexx build` (Windows local interpreter: `.tools/regina/rexx.exe`). Native utilities are in `rust/target/debug/`, with `.exe` suffix on Windows.

On the licensor's offline machine:

```text
david-license-admin issuer-key issuer-private.key issuer-public.hex
```

Keep the private file offline and outside distributed builds. Copy only the generated public hex into `config/licensing-root.hex`, review it, then rebuild the runtime. This pins the issuer at compile time. Do not regenerate the issuer for each customer. The utility refuses to overwrite files. Windows private issuer files are DPAPI protected; Linux files are mode 0600. Back up the signing authority using a separately secured procedure; machine/user-bound DPAPI files are not portable backups. Do not distribute the admin utility as a customer runtime component.

On the licensed deployment, create ignored `.licensing/` with access restricted to its operator, then:

```text
david-license-admin deployment-key .licensing/deployment.key
david-license-admin deployment-id
```

Send the printed deployment fingerprint to the licensor. The AES key stays on the deployment. The licensor prepares strict JSON claims containing these fields:

```text
version: 1
entitlement_id: unique issuer-controlled ID
licensee: licensed organization/person
product: SYNTHETIC-DAVID-CORP
deployment_sha256: printed deployment fingerprint
features: purchased subset of corporate.validation, qwen.responses, codex.launch
issued_at, not_before, expires_at: integer Unix seconds
payment:
  status: SETTLED
  receipt_id: reference to independently verified settlement
  evidence_sha256: lowercase SHA-256 of trusted settlement evidence
  paid_from, paid_through: integer Unix seconds covering the whole interval
```

Financial amounts and payment tokens do not belong in claims. Maximum offline lease is 24 hours from issue; expiry cannot exceed the paid-through date. After independently verifying payment, the licensor runs:

```text
david-license-admin sign issuer-private.key claims.json customer.signed-entitlement.json
```

Transfer the signed grant to its customer, never the issuer private key. On the customer deployment with the matching pinned public key:

```text
david-license-admin install customer.signed-entitlement.json .licensing/deployment.key .licensing/current.entitlement
```

Set `DAVID_ENTITLEMENT_FILE` to the encrypted entitlement's absolute path and `DAVID_DEPLOYMENT_KEY_FILE` to the local protected key's absolute path in the runtime's process environment. `david license-status` reports authorization without keys. Restart Qwen after configuring its environment; it rereads files on each inference. Renew by installing to a new file and atomically replacing the old entitlement using the deployment's approved administration procedure.

## Boundaries

This enforces payment-backed access in the supplied runtime. It does not execute bank payments, verify payment-provider webhooks, or implement pricing, seats, concurrency, usage billing, or immediate online revocation. Entitlement features do not grant banking permissions; principal policy remains independently required and denies everyone by default.

Offline revocation/nonpayment takes effect when the last short lease expires or an administrator removes it. Refunds cannot update an existing offline signature. The issuer must stop renewal or issue a replacement under the applicable agreement. System time is used; trusted-clock rollback protection is absent. Windows DPAPI adds user/device binding; Linux file protection is weaker. Cloned VM identities, privileged users, copied Linux keys, and source/binary patching prevent an absolute anti-copy guarantee. TPM-backed nonexportable keys, trusted time and an authenticated online issuer are future requirements for stronger clone/revocation enforcement.

Cryptographic APIs: [RustCrypto AES-GCM](https://docs.rs/aes-gcm/0.10.3/aes_gcm/) and [Ed25519 strict verification](https://docs.rs/ed25519-dalek/2.2.0/ed25519_dalek/struct.VerifyingKey.html#method.verify_strict).
