//! Actual GnuCOBOL -> production Rust cdylib integration. No licensing bypass.
//! An isolated source copy pins a TEST-ONLY issuer; tracked production root stays empty.
use super::*;
use std::{fs, path::PathBuf, process::Command};

fn copy_tree(source: &Path, target: &Path) {
    fs::create_dir_all(target).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target.join(entry.file_name()));
        } else {
            fs::copy(entry.path(), target.join(entry.file_name())).unwrap();
        }
    }
}
fn checked(command: &mut Command) {
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{command:?}\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
struct Fixture {
    root: PathBuf,
    policy: PathBuf,
    db: PathBuf,
    key: PathBuf,
    entitlement: PathBuf,
}
impl Fixture {
    fn call_bytes(&self, bytes: &[u8]) -> Value {
        let input = self.root.join("input.bin");
        let output = self.root.join("output.bin");
        fs::write(&input, bytes).unwrap();
        if output.exists() {
            fs::remove_file(&output).unwrap();
        }
        checked(
            Command::new(self.root.join("build/broker-runner"))
                .current_dir(&self.root)
                .env("DD_NATIVEIN", input)
                .env("DD_NATIVEOUT", &output)
                .env("DAVID_PRINCIPALS_FILE", &self.policy)
                .env("DAVID_AUDIT_DB", &self.db)
                .env("DAVID_DEPLOYMENT_KEY_FILE", &self.key)
                .env("DAVID_ENTITLEMENT_FILE", &self.entitlement)
                .env("LD_LIBRARY_PATH", self.root.join("rust/target/debug")),
        );
        NativeResult::decode_bytes(&fs::read(output).unwrap()).unwrap()
    }
    fn call(&self, request: &Value) -> Value {
        self.call_bytes(NativeRequest::encode(request).unwrap().bytes())
    }
    fn store(&self) -> Store {
        Store::open(self.db.to_str().unwrap()).unwrap()
    }
}
#[test]
#[ignore = "requires Linux GnuCOBOL; CI runs the real native broker explicitly"]
fn native_broker_roundtrip() {
    checked(Command::new("cobc").arg("--version"));
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let root = std::env::temp_dir().join(format!("david-native-{}", crate::new_id()));
    fs::create_dir_all(root.join("rust")).unwrap();
    for dir in [
        "control-plane",
        "execution-gate",
        "qwen-endpoint",
        "cloudflare-finops",
    ] {
        copy_tree(&source.join("rust").join(dir), &root.join("rust").join(dir));
    }
    for file in ["Cargo.toml", "Cargo.lock"] {
        fs::copy(source.join("rust").join(file), root.join("rust").join(file)).unwrap();
    }
    for dir in ["config", "cobol", "examples"] {
        copy_tree(&source.join(dir), &root.join(dir));
    }
    fs::copy(
        source.join("SNAPKITTY SOVEREIGN COMMERCIAL LICENSE"),
        root.join("SNAPKITTY SOVEREIGN COMMERCIAL LICENSE"),
    )
    .unwrap();
    let issuer = ed25519_dalek::SigningKey::from_bytes(&[9; 32])
        .verifying_key()
        .to_bytes();
    let hex: String = issuer.iter().map(|b| format!("{b:02x}")).collect();
    fs::write(root.join("config/licensing-root.hex"), hex).unwrap();
    // Build unchanged production verifier with the isolated test issuer compiled in.
    checked(
        Command::new("cargo")
            .args([
                "build",
                "--locked",
                "-j",
                "2",
                "-p",
                "david-control-plane",
                "--lib",
                "--bin",
                "david",
            ])
            .current_dir(root.join("rust"))
            .env("RUSTUP_TOOLCHAIN", "1.99.0")
            .env("CARGO_TARGET_DIR", root.join("rust/target")),
    );
    checked(
        Command::new(root.join("rust/target/debug/david"))
            .arg("broker-build")
            .current_dir(&root),
    );
    let fixture = Fixture {
        policy: root.join("policy.json"),
        db: root.join("audit.sqlite"),
        key: root.join("deployment.key"),
        entitlement: root.join("test.entitlement"),
        root,
    };
    david_execution_gate::create_deployment_key(&fixture.key).unwrap();
    let key = david_execution_gate::read_deployment_key(&fixture.key).unwrap();
    let now = david_execution_gate::now().unwrap();
    let deployment = david_execution_gate::deployment_id().unwrap();
    let claims = david_execution_gate::Claims {
        version: 1,
        entitlement_id: "TEST-ONLY-NATIVE-BROKER".into(),
        licensee: "TEST-ONLY".into(),
        product: david_execution_gate::PRODUCT.into(),
        deployment_sha256: deployment.clone(),
        features: vec!["corporate.validation".into()],
        issued_at: now,
        not_before: now,
        expires_at: now + 3600,
        payment: david_execution_gate::Payment {
            status: "SETTLED".into(),
            receipt_id: "TEST-ONLY-NOT-A-PAYMENT".into(),
            evidence_sha256: "b".repeat(64),
            paid_from: now,
            paid_through: now + 3600,
        },
    };
    let signed = david_execution_gate::sign(claims, &[9; 32], now).unwrap();
    let sealed = david_execution_gate::seal(&signed, &key, &issuer, &deployment, now).unwrap();
    david_execution_gate::write_new_private(&fixture.entitlement, &sealed).unwrap();
    let policy = json!({"demo-reviewer":{"agents":["DOCUMENT","LEDGER","MIG-VALID","COBOL-ANALYZER"],"permissions":["document:read","ledger:read","migration:read","legacy:cobol:read"]}});
    fs::write(&fixture.policy, canonical(&policy)).unwrap();
    let request = crate::demo_request();
    let response = fixture.call(&request);
    assert_eq!(response["status"], "COMPLETED", "{response}");
    assert_eq!(response["evidence"]["kind"], "SQLITE-PROVENANCE-V1");
    let saved: Value = serde_json::from_str(
        &fixture
            .store()
            .db
            .query_row(
                "SELECT response FROM requests WHERE id=?",
                [request["requestId"].as_str().unwrap()],
                |r| r.get::<_, String>(0),
            )
            .unwrap(),
    )
    .unwrap();
    assert_eq!(response["outputHash"], hash(&saved["output"]));
    assert_eq!(response["provenanceId"], saved["provenance"][0]["id"]);
    let before = fixture.store().verify().unwrap();
    assert_eq!(before["count"], 2);
    assert_eq!(fixture.call(&request), response);
    assert_eq!(fixture.store().verify().unwrap(), before);
    let mut denied = policy.clone();
    denied["demo-reviewer"]["permissions"] = json!([]);
    fs::write(&fixture.policy, canonical(&denied)).unwrap();
    assert_eq!(fixture.call(&request)["errorMessage"], "MISSING_PERMISSION");
    assert_eq!(fixture.store().verify().unwrap(), before);
    fs::write(&fixture.policy, canonical(&policy)).unwrap();
    let mut conflicting = request.clone();
    conflicting["payload"]["text"] = json!("different");
    assert_eq!(
        fixture.call(&conflicting)["errorMessage"],
        "REQUEST_ID_CONFLICT"
    );
    let mut second = crate::demo_request();
    let second_response = fixture.call(&second);
    assert_eq!(second_response["status"], "COMPLETED");
    assert_eq!(fixture.store().verify().unwrap()["count"], 4);
    assert_eq!(
        fixture.call_bytes(&[])["errorMessage"],
        "INPUT-RECORD-INVALID"
    );
    assert_eq!(
        fixture.call_bytes(&NativeRequest::encode(&second).unwrap().bytes()[..100])["errorMessage"],
        "INPUT-RECORD-INVALID"
    );
    second["requestId"] = json!(crate::new_id());
    let mut malformed = NativeRequest::encode(&second).unwrap();
    malformed.payload_length = *b"0000000x";
    assert_eq!(
        fixture.call_bytes(malformed.bytes())["errorMessage"],
        "INVALID-PAYLOAD-LENGTH"
    );
    malformed = NativeRequest::encode(&second).unwrap();
    malformed.abi = *b"SDABI002";
    assert_eq!(
        fixture.call_bytes(malformed.bytes())["errorMessage"],
        "ABI-VERSION-UNSUPPORTED"
    );
    malformed = NativeRequest::encode(&second).unwrap();
    malformed.payload[8191] = b'x';
    assert_eq!(
        fixture.call_bytes(malformed.bytes())["errorMessage"],
        "ABI_PAYLOAD_PADDING_INVALID"
    );
    let mut extra = NativeRequest::encode(&second).unwrap().bytes().to_vec();
    extra.extend_from_slice(NativeRequest::encode(&second).unwrap().bytes());
    assert_eq!(
        fixture.call_bytes(&extra)["errorMessage"],
        "INPUT-FRAMING-INVALID"
    );
    assert_eq!(fixture.store().verify().unwrap()["count"], 4);
    let mut cobol: Value = serde_json::from_slice(
        &fs::read(fixture.root.join("examples/cobol-inspect.json")).unwrap(),
    )
    .unwrap();
    cobol["requestor"] = json!("demo-reviewer");
    assert_eq!(fixture.call(&cobol)["status"], "COMPLETED");
    let source = json!({"id":"TEST-ONLY","location":"native integration","sha256":"a".repeat(64)});
    let mut ledger = json!({"requestId":crate::new_id(),"traceId":crate::new_id(),"graphId":crate::new_id(),"requestor":"demo-reviewer","type":"LEDGER-VALIDATE","payload":{"source":source,"journalId":crate::new_id(),"currency":"USD","entries":[{"entryId":crate::new_id(),"accountId":crate::new_id(),"debit":"900719925474099.1234","credit":"0.0000"},{"entryId":crate::new_id(),"accountId":crate::new_id(),"debit":"0.0000","credit":"900719925474099.1234"}]}});
    assert_eq!(fixture.call(&ledger)["status"], "COMPLETED");
    let (_, saved) = fixture
        .store()
        .cached(ledger["requestId"].as_str().unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(saved["output"]["debitUnits"], "9007199254740991234");
    assert_eq!(saved["output"]["posted"], false);
    ledger["requestId"] = json!(crate::new_id());
    ledger["payload"]["entries"][1]["credit"] = json!("900719925474099.1233");
    let escalated = fixture.call(&ledger);
    assert_eq!(escalated["status"], "ESCALATED", "{escalated}");
    assert_eq!(escalated["evidence"]["kind"], "AUDITED-REQUEST-V1");
    assert!(
        fixture
            .store()
            .frozen(ledger["graphId"].as_str().unwrap())
            .unwrap()
    );
    ledger["requestId"] = json!(crate::new_id());
    assert_eq!(fixture.call(&ledger)["errorMessage"], "GRAPH_FROZEN");
    let mut migration = json!({"requestId":crate::new_id(),"traceId":crate::new_id(),"graphId":crate::new_id(),"requestor":"demo-reviewer","type":"MIGRATION-VALIDATE","payload":{"source":source,"modernSource":source,"legacy":{"total":"1.0000"},"modern":{"total":"1.0000"},"equivalence":"EXACT-CANONICAL-JSON-V1"}});
    assert_eq!(fixture.call(&migration)["status"], "COMPLETED");
    let (_, saved) = fixture
        .store()
        .cached(migration["requestId"].as_str().unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(saved["migrationApproved"], false);
    migration["requestId"] = json!(crate::new_id());
    migration["payload"]["modern"]["total"] = json!("2.0000");
    let diverged = fixture.call(&migration);
    assert_eq!(diverged["status"], "ESCALATED");
    assert_eq!(diverged["errorMessage"], "MIGRATION_DIVERGENCE");
    assert!(
        fixture
            .store()
            .frozen(migration["graphId"].as_str().unwrap())
            .unwrap()
    );
    let fault_before = fixture.store().verify().unwrap();
    fixture.store().db.execute_batch("CREATE TRIGGER test_fail BEFORE INSERT ON events BEGIN SELECT RAISE(ABORT,'test rollback'); END;").unwrap();
    assert_eq!(
        fixture.call(&crate::demo_request())["errorMessage"],
        "PERSISTENCE_FAILURE"
    );
    assert_eq!(fixture.store().verify().unwrap(), fault_before);
    fixture
        .store()
        .db
        .execute_batch("DROP TRIGGER test_fail;")
        .unwrap();
    fixture
        .store()
        .db
        .execute(
            "UPDATE requests SET response='{}' WHERE id=?",
            [request["requestId"].as_str().unwrap()],
        )
        .unwrap();
    assert_eq!(
        fixture.call(&request)["errorMessage"],
        "CACHE_INTEGRITY_INVALID"
    );
    assert_eq!(fixture.store().verify().unwrap(), fault_before);
    // Prove the normal tracked build has no authority from this fixture root.
    fs::write(
        fixture.root.join("config/licensing-root.hex"),
        fs::read(source_path()).unwrap(),
    )
    .unwrap();
    checked(
        Command::new("cargo")
            .args([
                "build",
                "--locked",
                "-j",
                "2",
                "-p",
                "david-control-plane",
                "--lib",
            ])
            .current_dir(fixture.root.join("rust"))
            .env("RUSTUP_TOOLCHAIN", "1.99.0")
            .env("CARGO_TARGET_DIR", fixture.root.join("rust/target")),
    );
    if david_execution_gate::issuer_root().is_err() {
        assert_eq!(
            fixture.call(&crate::demo_request())["errorMessage"],
            "LICENSE_ISSUER_UNCONFIGURED"
        );
        assert_eq!(fixture.store().verify().unwrap(), fault_before);
    }
    println!(
        "PASS: real COBOL/Rust ABI, licensed validation, replay, revocation, restart, exact money, escalation, rollback, framing and cache integrity"
    );
}
fn source_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("config/licensing-root.hex")
}
