use david_control_plane::*;
use serde_json::{Value, json};
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
