use crate::*;
use serde_json::{Value, json};
// Test-only access to the deterministic core. Production exposes only gated execute.
fn execute(store: &Store, policy: &Value, request: &Value) -> Value {
    crate::execute_inner(store, policy, request, None)
}
#[test]
fn licensing_blocks_cache_replay_and_creates_no_audit_on_denial() {
    let s = store();
    let request = ledger();
    execute(&s, &policy(), &request);
    let before = s.verify().unwrap();
    if david_execution_gate::issuer_root().is_err() {
        error(
            &crate::execute(&s, &policy(), &request),
            "LICENSE_ISSUER_UNCONFIGURED",
        );
        assert_eq!(s.verify().unwrap(), before);
    }
    let time = david_execution_gate::now().unwrap();
    let claims = david_execution_gate::Claims {
        version: 1,
        entitlement_id: "EXPIRED-DEVELOPMENT-FIXTURE".into(),
        licensee: "TEST-ONLY".into(),
        product: david_execution_gate::PRODUCT.into(),
        deployment_sha256: "a".repeat(64),
        features: vec!["corporate.validation".into()],
        issued_at: time - 1000,
        not_before: time - 1000,
        expires_at: time - 100,
        payment: david_execution_gate::Payment {
            status: "SETTLED".into(),
            receipt_id: "TEST-ONLY-NOT-A-PAYMENT".into(),
            evidence_sha256: "b".repeat(64),
            paid_from: time - 1000,
            paid_through: time - 100,
        },
    };
    let mut active = claims.clone();
    active.expires_at = time + 500;
    active.payment.paid_through = time + 500;
    let signed = david_execution_gate::sign(claims, &[9; 32], time - 500).unwrap();
    // Public key of the explicit test seed; no production signing authority.
    let issuer = ed25519_dalek::SigningKey::from_bytes(&[9; 32])
        .verifying_key()
        .to_bytes();
    let sealed =
        david_execution_gate::seal(&signed, &[7; 32], &issuer, &"a".repeat(64), time - 500)
            .unwrap();
    let permit =
        david_execution_gate::verify(&sealed, &[7; 32], &issuer, &"a".repeat(64), time - 500)
            .unwrap();
    error(
        &crate::execute_inner(&s, &policy(), &request, Some(&permit)),
        "LICENSE_EXPIRED",
    );
    assert_eq!(s.verify().unwrap(), before);
    let signed = david_execution_gate::sign(active, &[9; 32], time).unwrap();
    let sealed =
        david_execution_gate::seal(&signed, &[7; 32], &issuer, &"a".repeat(64), time).unwrap();
    let permit =
        david_execution_gate::verify(&sealed, &[7; 32], &issuer, &"a".repeat(64), time).unwrap();
    let mut fresh = ledger();
    let result = crate::execute_inner(&s, &policy(), &fresh, Some(&permit));
    assert_eq!(result["status"], "COMPLETED");
    assert_eq!(result["executionEntitlement"]["authorized"], true);
    assert_eq!(s.verify().unwrap()["count"], 4);
    let mut denied = policy();
    denied["reviewer"]["permissions"] = json!([]);
    fresh["requestId"] = json!(new_id());
    error(
        &crate::execute_inner(&s, &denied, &fresh, Some(&permit)),
        "MISSING_PERMISSION",
    );
}
fn policy() -> Value {
    json!({"reviewer":{"agents":["LEDGER","DOCUMENT","MIG-VALID"],"permissions":["ledger:read","document:read","migration:read"]}})
}
fn source() -> Value {
    json!({"id":"TEST-FIXTURE","location":"tests/controls.rs","sha256":"a".repeat(64)})
}
fn ledger() -> Value {
    json!({"requestId":new_id(),"traceId":new_id(),"graphId":new_id(),"requestor":"reviewer","type":"LEDGER-VALIDATE","payload":{"source":source(),"journalId":new_id(),"currency":"USD","entries":[{"entryId":new_id(),"accountId":new_id(),"debit":"900719925474099.1234","credit":"0.0000"},{"entryId":new_id(),"accountId":new_id(),"debit":"0.0000","credit":"900719925474099.1234"}]}})
}
fn store() -> Store {
    Store::open(":memory:").unwrap()
}
fn error(r: &Value, code: &str) {
    assert_eq!(r["errorCode"], code, "{r}");
    assert_ne!(r["status"], "COMPLETED");
}
#[test]
fn minimum() {
    assert_eq!(
        select_minimum(
            &["ledger.validate"],
            &json!(["DOCUMENT", "LEDGER"]),
            &json!(["document:read", "ledger:read"])
        )
        .unwrap(),
        vec!["LEDGER"]
    );
}
#[test]
fn exact_money() {
    let s = store();
    let r = execute(&s, &policy(), &ledger());
    assert_eq!(r["output"]["debitUnits"], "9007199254740991234");
    assert_eq!(r["output"]["posted"], false);
    assert_eq!(r["provenance"].as_array().unwrap().len(), 1);
    assert_eq!(s.verify().unwrap()["count"], 2);
}
#[test]
fn invalid_amounts() {
    for v in [
        json!(1),
        json!("1e4"),
        json!("-1.0000"),
        json!("1.00000"),
        json!("1000000000000000.0000"),
    ] {
        assert_eq!(amount(&v).unwrap_err(), "INVALID_AMOUNT");
    }
}
#[test]
fn escalation_and_restart() {
    let path = std::env::temp_dir().join(format!("david-{}.sqlite", new_id()));
    let pathstr = path.to_str().unwrap();
    let mut req = ledger();
    req["payload"]["entries"][1]["credit"] = json!("900719925474099.1233");
    {
        let s = Store::open(pathstr).unwrap();
        let r = execute(&s, &policy(), &req);
        error(&r, "LEDGER_MISMATCH");
        assert_eq!(r["status"], "ESCALATED");
    }
    {
        let s = Store::open(pathstr).unwrap();
        assert!(s.frozen(req["graphId"].as_str().unwrap()).unwrap());
        req["requestId"] = json!(new_id());
        error(&execute(&s, &policy(), &req), "GRAPH_FROZEN");
    }
    std::fs::remove_file(path).unwrap();
}
#[test]
fn replay_and_conflict() {
    let s = store();
    let mut q = ledger();
    let r = execute(&s, &policy(), &q);
    assert_eq!(execute(&s, &policy(), &q), r);
    assert_eq!(s.verify().unwrap()["count"], 2);
    q["payload"]["currency"] = json!("EUR");
    error(&execute(&s, &policy(), &q), "REQUEST_ID_CONFLICT");
    assert_eq!(s.verify().unwrap()["count"], 2);
}
#[test]
fn permissions_and_agents() {
    let mut p = policy();
    p["reviewer"]["permissions"] = json!([]);
    error(&execute(&store(), &p, &ledger()), "MISSING_PERMISSION");
    p["reviewer"]["agents"] = json!(["GHOST"]);
    error(&execute(&store(), &p, &ledger()), "UNKNOWN_AGENT");
}
#[test]
fn unknown_tools() {
    for tool in ["PAYMENT-ORDER", "BASH", "CURL", "MIGRATION-APPROVE"] {
        let mut q = ledger();
        q["type"] = json!(tool);
        error(&execute(&store(), &policy(), &q), "UNKNOWN_TOOL");
    }
}
#[test]
fn bad_evidence() {
    let mut q = ledger();
    q["payload"].as_object_mut().unwrap().remove("source");
    error(&execute(&store(), &policy(), &q), "SCHEMA_MISMATCH");
    let mut q = ledger();
    q["payload"]["entries"][1]["entryId"] = q["payload"]["entries"][0]["entryId"].clone();
    error(&execute(&store(), &policy(), &q), "DUPLICATE_ENTRY");
    let mut q = ledger();
    q["payload"]["entries"][0]["credit"] = json!("1.0000");
    error(&execute(&store(), &policy(), &q), "INVALID_ENTRY");
}
#[test]
fn secrets_never_persist() {
    for secret in [json!({"apiKey":"test"}), json!("Bearer fixture")] {
        let s = store();
        let mut q = ledger();
        q["payload"]["secretFixture"] = secret;
        error(&execute(&s, &policy(), &q), "SECRET_FIELD_FORBIDDEN");
        assert_eq!(s.verify().unwrap()["count"], 0);
    }
}
#[test]
fn identity_and_size() {
    let s = store();
    let mut q = ledger();
    q.as_object_mut().unwrap().remove("traceId");
    error(&execute(&s, &policy(), &q), "SCHEMA_MISMATCH");
    let mut q = ledger();
    q["payload"] = json!("a".repeat(8193));
    error(&execute(&s, &policy(), &q), "PAYLOAD_TOO_LARGE");
    assert_eq!(s.verify().unwrap()["count"], 0);
}
#[test]
fn document_hash() {
    let mut q = demo_request();
    let s = store();
    let r = execute(&s, &demo_policy(), &q);
    assert_eq!(r["output"]["lines"], 2);
    assert_eq!(r["output"]["summaryGenerated"], false);
    q["requestId"] = json!(new_id());
    q["payload"]["text"] = json!("changed");
    error(&execute(&s, &demo_policy(), &q), "SOURCE_HASH_MISMATCH");
}
#[test]
fn migration() {
    let mut q = ledger();
    q["type"] = json!("MIGRATION-VALIDATE");
    q["payload"] = json!({"source":source(),"modernSource":source(),"legacy":{"a":1,"b":2},"modern":{"b":2,"a":1},"equivalence":"EXACT-CANONICAL-JSON-V1"});
    let s = store();
    let r = execute(&s, &policy(), &q);
    assert_eq!(r["output"]["sourceOfTruth"], "LEGACY");
    assert_eq!(r["migrationApproved"], false);
    q["requestId"] = json!(new_id());
    q["payload"]["modern"]["a"] = json!(2);
    let r = execute(&s, &policy(), &q);
    error(&r, "MIGRATION_DIVERGENCE");
    assert_eq!(r["status"], "ESCALATED");
}
#[test]
fn reopen_preserves_chain() {
    let path = std::env::temp_dir().join(format!("david-{}.sqlite", new_id()));
    let pathstr = path.to_str().unwrap();
    let head;
    {
        let s = Store::open(pathstr).unwrap();
        execute(&s, &policy(), &ledger());
        head = s.verify().unwrap()["head"].clone();
    }
    {
        let s = Store::open(pathstr).unwrap();
        assert_eq!(s.verify().unwrap()["head"], head);
        execute(&s, &policy(), &ledger());
        assert_eq!(s.verify().unwrap()["count"], 4);
    }
    std::fs::remove_file(path).unwrap();
}
#[test]
fn tamper_detected() {
    let s = store();
    execute(&s, &policy(), &ledger());
    assert!(s.db.execute("UPDATE events SET body='{}'", []).is_err());
    assert!(s.db.execute("DELETE FROM provenance", []).is_err());
    s.db.execute_batch(
        "DROP TRIGGER immutable_provenance_update; UPDATE provenance SET body='{}';",
    )
    .unwrap();
    assert_eq!(s.verify().unwrap_err(), "PROVENANCE_INVALID");
    error(&execute(&s, &policy(), &ledger()), "PROVENANCE_INVALID");
}
#[test]
fn atomic_rollback() {
    let s = store();
    s.db.execute_batch(
        "CREATE TRIGGER failure BEFORE INSERT ON events BEGIN SELECT RAISE(ABORT,'fixture'); END;",
    )
    .unwrap();
    error(&execute(&s, &policy(), &ledger()), "PERSISTENCE_FAILURE");
    assert_eq!(
        s.db.query_row("SELECT COUNT(*) FROM provenance", [], |r| r
            .get::<_, i32>(0))
            .unwrap(),
        0
    );
    assert_eq!(s.verify().unwrap()["count"], 0);
}
#[test]
fn canonical_compatibility() {
    assert_eq!(
        canonical(&json!({"z":1.0,"a":"text"})),
        r#"{"a":"text","z":1}"#
    );
    assert_eq!(canonical(&json!(1e-7)), "1e-7");
}

fn cobol_request(text: &str, dialect: &str) -> Value {
    let mut request = demo_request();
    request["requestor"] = json!("legacy-reviewer");
    request["type"] = json!("COBOL-INSPECT");
    request["payload"] = json!({"source":{"id":"DEVELOPMENT-COBOL-FIXTURE","location":"tests/controls.rs","sha256":format!("{:x}",Sha256::digest(text.as_bytes()))},"text":text,"dialect":dialect});
    request
}
fn cobol_policy() -> Value {
    json!({"legacy-reviewer":{"agents":["DOCUMENT","COBOL-ANALYZER"],"permissions":["document:read","legacy:cobol:read"]}})
}
const FREE_COBOL: &str = "IDENTIFICATION DIVISION.\nPROGRAM-ID. FIXTURE.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 AMOUNT PIC 9(4).\nPROCEDURE DIVISION.\nMAIN-PARA.\nDISPLAY 'FAKE SECTION. COPY X.'. *> FALSE DIVISION.\nGOBACK.\n";
#[test]
fn cobol_free_inventory_and_provenance() {
    let s = store();
    let request = cobol_request(FREE_COBOL, "COBOL85-FREE");
    let result = execute(&s, &cobol_policy(), &request);
    assert_eq!(result["status"], "COMPLETED", "{result}");
    assert_eq!(result["agents"], json!(["COBOL-ANALYZER"]));
    let output = &result["output"];
    assert_eq!(output["version"], 1);
    assert_eq!(output["programs"][0]["name"], "FIXTURE");
    assert_eq!(output["paragraphs"].as_array().unwrap().len(), 1);
    assert_eq!(output["paragraphs"][0]["name"], "MAIN-PARA");
    assert_eq!(output["dataItems"][0]["name"], "AMOUNT");
    assert_eq!(
        output["dataItems"][0]["span"],
        json!({"start":{"line":5,"column":1},"end":{"line":5,"column":10}})
    );
    assert_eq!(output["compilerValidated"], false);
    assert_eq!(
        output["sourceSha256"],
        request["payload"]["source"]["sha256"]
    );
    assert_eq!(
        result["provenance"][0]["sources"][0],
        request["payload"]["source"]
    );
    assert_eq!(result["provenance"][0]["toolId"], "ANALYZE-COBOL");
    assert_eq!(s.verify().unwrap()["count"], 2);
    assert_eq!(execute(&s, &cobol_policy(), &request), result);
}
#[test]
fn cobol_fixed_columns_comments_and_crlf() {
    let fixed = "000100 IDENTIFICATION DIVISION.\r\n000200 PROGRAM-ID. FIXTURE.\r\n000300*PROGRAM-ID. FAKE.\r\n000400 PROCEDURE DIVISION USING INPUT-AREA.\r\n000500 MAIN-PARA.\r\n000600     DISPLAY 'DON''T COPY X.'.\r\n000700     GOBACK.\r\n";
    let result = execute(
        &store(),
        &cobol_policy(),
        &cobol_request(fixed, "COBOL85-FIXED"),
    );
    assert_eq!(result["status"], "COMPLETED", "{result}");
    assert_eq!(result["output"]["programs"].as_array().unwrap().len(), 1);
    assert_eq!(
        result["output"]["paragraphs"][0]["span"]["start"],
        json!({"line":5,"column":8})
    );
    let padded = format!(
        "       IDENTIFICATION DIVISION.{}IGNORED\n       PROGRAM-ID. FIXTURE.\n",
        " ".repeat(72 - 31)
    );
    let result = execute(
        &store(),
        &cobol_policy(),
        &cobol_request(&padded, "COBOL85-FIXED"),
    );
    assert_eq!(result["status"], "COMPLETED", "{result}");
    assert_eq!(
        result["output"]["diagnostics"][0]["code"],
        "FIXED_IDENTIFICATION_AREA_IGNORED"
    );
}
#[test]
fn cobol_scope_terminators_are_not_paragraphs() {
    let text = format!("{FREE_COBOL}END-ADD.\nEND-READ.\nEND-WRITE.\nCONTINUE.\nEND-DIVIDE.\n");
    let result = execute(
        &store(),
        &cobol_policy(),
        &cobol_request(&text, "COBOL85-FREE"),
    );
    assert_eq!(result["status"], "COMPLETED");
    assert_eq!(result["output"]["paragraphs"].as_array().unwrap().len(), 1);
}
#[test]
fn cobol_rejects_unsupported_source_instead_of_inventing_semantics() {
    for (text, dialect, code) in [
        (FREE_COBOL, "AUTO", "UNKNOWN_DIALECT"),
        (
            "       IDENTIFICATION DIVISION.\n      -CONTINUATION",
            "COBOL85-FIXED",
            "UNSUPPORTED_CONTINUATION",
        ),
        (
            "      DDISPLAY X.",
            "COBOL85-FIXED",
            "UNSUPPORTED_DEBUG_LINE",
        ),
        (
            "IDENTIFICATION DIVISION.",
            "COBOL85-FIXED",
            "INVALID_FIXED_FORMAT",
        ),
        (
            "PROGRAM-ID. X.\nDISPLAY 'unterminated",
            "COBOL85-FREE",
            "UNSUPPORTED_MULTILINE_LITERAL",
        ),
        (
            "IDENTIFICATION DIVISION.\nPROGRAM-ID. X.\nCOPY FILE.",
            "COBOL85-FREE",
            "UNSUPPORTED_PREPROCESSING",
        ),
        (
            "IDENTIFICATION DIVISION.\nPROGRAM-ID. X.\n>>SOURCE FORMAT FREE",
            "COBOL85-FREE",
            "UNSUPPORTED_PREPROCESSING",
        ),
        (
            "IDENTIFICATION DIVISION.\nPROGRAM-ID. X.\n\tDISPLAY X.",
            "COBOL85-FREE",
            "UNSUPPORTED_SOURCE_ENCODING",
        ),
        (
            "IDENTIFICATION DIVISION.\nPROGRAM-ID. X.\nDISPLAY 'é'.",
            "COBOL85-FREE",
            "UNSUPPORTED_SOURCE_ENCODING",
        ),
        (
            "IDENTIFICATION DIVISION.\rPROGRAM-ID. X.",
            "COBOL85-FREE",
            "UNSUPPORTED_LINE_ENDING",
        ),
        (
            "IDENTIFICATION DIVISION.\nPROGRAM-ID. X.\nPROGRAM-ID. Y.",
            "COBOL85-FREE",
            "UNSUPPORTED_PROGRAM_STRUCTURE",
        ),
    ] {
        let result = execute(&store(), &cobol_policy(), &cobol_request(text, dialect));
        error(&result, code);
    }
}
#[test]
fn cobol_hash_permissions_schema_and_admission_fail_closed() {
    let mut request = cobol_request(FREE_COBOL, "COBOL85-FREE");
    let mut policy = cobol_policy();
    policy["legacy-reviewer"]["permissions"] = json!(["document:read"]);
    error(&execute(&store(), &policy, &request), "MISSING_PERMISSION");
    request["payload"]["text"] = json!(format!("{FREE_COBOL}\n"));
    error(
        &execute(&store(), &cobol_policy(), &request),
        "SOURCE_HASH_MISMATCH",
    );
    request["payload"]["source"]["sha256"] = json!(hash(&request["payload"]["text"]));
    error(
        &execute(&store(), &cobol_policy(), &request),
        "SOURCE_HASH_MISMATCH",
    );
    request["payload"]["unexpected"] = json!(true);
    error(
        &execute(&store(), &cobol_policy(), &request),
        "SCHEMA_MISMATCH",
    );
    let s = store();
    let mut request = cobol_request(&" ".repeat(8200), "COBOL85-FREE");
    error(&execute(&s, &cobol_policy(), &request), "PAYLOAD_TOO_LARGE");
    request["payload"]["text"] = json!("DISPLAY 'Bearer fixture'.");
    error(
        &execute(&s, &cobol_policy(), &request),
        "SECRET_FIELD_FORBIDDEN",
    );
    assert_eq!(s.verify().unwrap()["count"], 0);
}
#[test]
fn cached_evidence_requires_current_principal_authority() {
    let s = store();
    let request = cobol_request(FREE_COBOL, "COBOL85-FREE");
    let result = execute(&s, &cobol_policy(), &request);
    assert_eq!(result["status"], "COMPLETED");
    let before = s.verify().unwrap();
    error(&execute(&s, &json!({}), &request), "UNKNOWN_REQUESTOR");
    let mut policy = cobol_policy();
    policy["legacy-reviewer"]["permissions"] = json!([]);
    error(&execute(&s, &policy, &request), "MISSING_PERMISSION");
    policy["legacy-reviewer"]["agents"] = json!(["DOCUMENT"]);
    error(&execute(&s, &policy, &request), "MISSING_PERMISSION");
    let mut changed = request.clone();
    changed["payload"]["dialect"] = json!("COBOL85-FIXED");
    error(&execute(&s, &json!({}), &changed), "REQUEST_ID_CONFLICT");
    assert_eq!(s.verify().unwrap(), before);
    assert_eq!(execute(&s, &cobol_policy(), &request), result);
}

#[test]
fn published_cobol_example_works_with_verified_test_only_entitlement() {
    let time = david_execution_gate::now().unwrap();
    let claims = david_execution_gate::Claims {
        version: 1,
        entitlement_id: "DEVELOPMENT-COBOL-ENTITLEMENT-FIXTURE".into(),
        licensee: "TEST-ONLY".into(),
        product: david_execution_gate::PRODUCT.into(),
        deployment_sha256: "a".repeat(64),
        features: vec!["corporate.validation".into()],
        issued_at: time,
        not_before: time,
        expires_at: time + 500,
        payment: david_execution_gate::Payment {
            status: "SETTLED".into(),
            receipt_id: "TEST-ONLY-NOT-A-PAYMENT".into(),
            evidence_sha256: "b".repeat(64),
            paid_from: time,
            paid_through: time + 500,
        },
    };
    let issuer = ed25519_dalek::SigningKey::from_bytes(&[9; 32])
        .verifying_key()
        .to_bytes();
    let signed = david_execution_gate::sign(claims, &[9; 32], time).unwrap();
    let sealed =
        david_execution_gate::seal(&signed, &[7; 32], &issuer, &"a".repeat(64), time).unwrap();
    let permit =
        david_execution_gate::verify(&sealed, &[7; 32], &issuer, &"a".repeat(64), time).unwrap();
    let request: Value =
        serde_json::from_str(include_str!("../../../examples/cobol-inspect.json")).unwrap();
    let policy: Value =
        serde_json::from_str(include_str!("../../../examples/cobol-principal.json")).unwrap();
    let s = store();
    let result = crate::execute_inner(&s, &policy, &request, Some(&permit));
    assert_eq!(result["status"], "COMPLETED", "{result}");
    assert_eq!(result["executionEntitlement"]["authorized"], true);
    assert_eq!(result["output"]["programs"][0]["name"], "INSPECT-FIXTURE");
    assert_eq!(result["output"]["dataItems"][0]["name"], "INPUT-COUNT");
    assert_eq!(s.verify().unwrap()["count"], 2);
    assert!(!s.frozen(request["graphId"].as_str().unwrap()).unwrap());
}
