use chrono::{SecondsFormat, Utc};
use regex::Regex;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use uuid::Uuid;
mod cobol_inspection;

pub type Result<T> = std::result::Result<T, String>;
fn ensure(ok: bool, code: &str) -> Result<()> {
    if ok { Ok(()) } else { Err(code.into()) }
}
fn matches(pattern: &str, value: &Value) -> bool {
    value
        .as_str()
        .is_some_and(|s| Regex::new(pattern).unwrap().is_match(s))
}
pub fn canonical(v: &Value) -> String {
    match v {
        Value::Object(m) => {
            let mut keys: Vec<_> = m.keys().collect();
            keys.sort_by_key(|k| k.encode_utf16().collect::<Vec<_>>());
            format!(
                "{{{}}}",
                keys.into_iter()
                    .map(|k| format!("{}:{}", json!(k), canonical(&m[k])))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        }
        Value::Array(a) => format!(
            "[{}]",
            a.iter().map(canonical).collect::<Vec<_>>().join(",")
        ),
        Value::Number(n) if n.is_f64() => {
            ryu_js::Buffer::new().format(n.as_f64().unwrap()).to_owned()
        }
        _ => v.to_string(),
    }
}
pub fn hash(v: &Value) -> String {
    format!("{:x}", Sha256::digest(canonical(v).as_bytes()))
}
fn object(v: &Value, keys: &[&str]) -> Result<()> {
    ensure(
        v.as_object()
            .is_some_and(|m| m.len() == keys.len() && keys.iter().all(|k| m.contains_key(*k))),
        "SCHEMA_MISMATCH",
    )
}
fn text(v: &Value, max: usize) -> Result<()> {
    ensure(
        v.as_str()
            .is_some_and(|s| !s.trim().is_empty() && s.encode_utf16().count() <= max),
        "SCHEMA_MISMATCH",
    )
}
fn id(v: &Value) -> Result<()> {
    ensure(
        matches(r"^[0-9a-f]{8}(-[0-9a-f]{4}){3}-[0-9a-f]{12}$", v),
        "SCHEMA_MISMATCH",
    )
}
pub fn amount(v: &Value) -> Result<u128> {
    ensure(
        matches(r"^(0|[1-9][0-9]{0,14})\.[0-9]{4}$", v),
        "INVALID_AMOUNT",
    )?;
    v.as_str()
        .unwrap()
        .replace('.', "")
        .parse()
        .map_err(|_| "INVALID_AMOUNT".into())
}
fn source(v: &Value) -> Result<()> {
    object(v, &["id", "location", "sha256"])?;
    text(&v["id"], 64)?;
    text(&v["location"], 128)?;
    ensure(
        matches(r"^[a-f0-9]{64}$", &v["sha256"]),
        "PROVENANCE_MISSING",
    )
}
fn no_secrets(v: &Value) -> Result<()> {
    match v {
        Value::Object(m) => {
            for (k, v) in m {
                ensure(
                    !Regex::new(r"(?i)password|secret|token|authorization|api.?key|private.?key")
                        .unwrap()
                        .is_match(k),
                    "SECRET_FIELD_FORBIDDEN",
                )?;
                no_secrets(v)?;
            }
        }
        Value::Array(a) => {
            for v in a {
                no_secrets(v)?;
            }
        }
        Value::String(_) => ensure(
            !matches(
                r"(?i)-----BEGIN .*PRIVATE KEY-----|\bBearer\s+\S+|\bsk-[A-Za-z0-9_-]{16,}",
                v,
            ),
            "SECRET_FIELD_FORBIDDEN",
        )?,
        _ => (),
    }
    Ok(())
}
fn admit(v: &Value) -> Result<()> {
    object(
        v,
        &[
            "requestId",
            "traceId",
            "graphId",
            "requestor",
            "type",
            "payload",
        ],
    )?;
    for k in ["requestId", "traceId", "graphId"] {
        id(&v[k])?;
    }
    text(&v["requestor"], 64)?;
    text(&v["type"], 32)?;
    no_secrets(&v["payload"])?;
    ensure(canonical(&v["payload"]).len() <= 8192, "PAYLOAD_TOO_LARGE")
}
fn role(agent: &str) -> Option<(&'static str, &'static str, &'static str)> {
    match agent {
        "DOCUMENT" => Some(("document.inspect", "document:read", "DOCUMENT-PARSE")),
        "COBOL-ANALYZER" => Some(("legacy.cobol.inspect", "legacy:cobol:read", "ANALYZE-COBOL")),
        "LEDGER" => Some(("ledger.validate", "ledger:read", "DOUBLE-ENTRY-VALIDATE")),
        "MIG-VALID" => Some(("migration.validate", "migration:read", "LEGACY-MODERN-DIFF")),
        _ => None,
    }
}
pub fn select_minimum(
    capabilities: &[&str],
    agents: &Value,
    permissions: &Value,
) -> Result<Vec<String>> {
    let agents = agents.as_array().ok_or("UNKNOWN_AGENT")?;
    ensure(
        agents.len() <= 64
            && agents
                .iter()
                .all(|a| a.as_str().is_some_and(|s| role(s).is_some())),
        "UNKNOWN_AGENT",
    )?;
    let set: BTreeSet<_> = agents.iter().map(|a| a.as_str().unwrap()).collect();
    ensure(set.len() == agents.len(), "SCHEMA_MISMATCH")?;
    let permissions = permissions.as_array().ok_or("SCHEMA_MISMATCH")?;
    ensure(permissions.iter().all(Value::is_string), "SCHEMA_MISMATCH")?;
    let eligible: Vec<_> = set
        .into_iter()
        .filter(|a| permissions.iter().any(|p| p == role(a).unwrap().1))
        .collect();
    let mut solutions = Vec::new();
    for mask in 1..(1 << eligible.len()) {
        let selected: Vec<String> = eligible
            .iter()
            .enumerate()
            .filter(|(i, _)| mask & (1 << i) != 0)
            .map(|(_, a)| a.to_string())
            .collect();
        if capabilities
            .iter()
            .all(|c| selected.iter().any(|a| role(a).unwrap().0 == *c))
        {
            solutions.push(selected);
        }
    }
    solutions.sort_by(|a, b| a.len().cmp(&b.len()).then(a.cmp(b)));
    solutions
        .into_iter()
        .next()
        .ok_or("MISSING_PERMISSION".into())
}
fn validate(agent: &str, p: &Value) -> Result<Value> {
    match agent {
        "COBOL-ANALYZER" => {
            object(p, &["source", "text", "dialect"])?;
            source(&p["source"])?;
            text(&p["text"], 7000)?;
            cobol_inspection::inspect(p)
        }
        "DOCUMENT" => {
            object(p, &["source", "text"])?;
            source(&p["source"])?;
            text(&p["text"], 7000)?;
            ensure(
                hash(&p["text"]) == p["source"]["sha256"],
                "SOURCE_HASH_MISMATCH",
            )?;
            let s = p["text"].as_str().unwrap();
            Ok(
                json!({"characters":s.encode_utf16().count(),"lines":s.split('\n').count(),"summaryGenerated":false}),
            )
        }
        "MIG-VALID" => {
            object(
                p,
                &["source", "modernSource", "legacy", "modern", "equivalence"],
            )?;
            source(&p["source"])?;
            source(&p["modernSource"])?;
            ensure(
                p["equivalence"] == "EXACT-CANONICAL-JSON-V1",
                "UNKNOWN_EQUIVALENCE",
            )?;
            ensure(
                canonical(&p["legacy"]) == canonical(&p["modern"]),
                "MIGRATION_DIVERGENCE",
            )?;
            Ok(
                json!({"equivalent":true,"specification":p["equivalence"],"migrationApproved":false,"sourceOfTruth":"LEGACY"}),
            )
        }
        "LEDGER" => {
            object(p, &["source", "journalId", "currency", "entries"])?;
            source(&p["source"])?;
            id(&p["journalId"])?;
            ensure(matches(r"^[A-Z]{3}$", &p["currency"]), "SCHEMA_MISMATCH")?;
            let entries = p["entries"].as_array().ok_or("SCHEMA_MISMATCH")?;
            ensure((2..=256).contains(&entries.len()), "SCHEMA_MISMATCH")?;
            let (mut debits, mut credits) = (0u128, 0u128);
            let mut ids = BTreeSet::new();
            for e in entries {
                object(e, &["entryId", "accountId", "debit", "credit"])?;
                id(&e["entryId"])?;
                id(&e["accountId"])?;
                ensure(
                    ids.insert(e["entryId"].as_str().unwrap()),
                    "DUPLICATE_ENTRY",
                )?;
                let (d, c) = (amount(&e["debit"])?, amount(&e["credit"])?);
                ensure((d > 0 && c == 0) || (c > 0 && d == 0), "INVALID_ENTRY")?;
                debits += d;
                credits += c;
            }
            ensure(debits == credits, "LEDGER_MISMATCH")?;
            Ok(
                json!({"journalId":p["journalId"],"currency":p["currency"],"debitUnits":debits.to_string(),"creditUnits":credits.to_string(),"scale":4,"balanced":true,"posted":false}),
            )
        }
        _ => Err("UNKNOWN_AGENT".into()),
    }
}
fn db_error(_: rusqlite::Error) -> String {
    "PERSISTENCE_FAILURE".into()
}
pub struct Store {
    pub db: Connection,
}
impl Store {
    pub fn open(filename: &str) -> Result<Self> {
        if filename != ":memory:" {
            if let Some(p) = std::path::Path::new(filename)
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
            {
                std::fs::create_dir_all(p).map_err(|_| "PERSISTENCE_FAILURE")?;
            }
        }
        let store = Self {
            db: Connection::open(filename).map_err(db_error)?,
        };
        store.db.execute_batch("PRAGMA foreign_keys=ON; PRAGMA busy_timeout=5000;
          CREATE TABLE IF NOT EXISTS events(seq INTEGER PRIMARY KEY,body TEXT NOT NULL,digest TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS provenance(id TEXT PRIMARY KEY,body TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS graphs(id TEXT PRIMARY KEY,frozen INTEGER NOT NULL);
          CREATE TABLE IF NOT EXISTS requests(id TEXT PRIMARY KEY,digest TEXT NOT NULL,response TEXT NOT NULL);
          CREATE TRIGGER IF NOT EXISTS immutable_events_update BEFORE UPDATE ON events BEGIN SELECT RAISE(ABORT,'immutable audit'); END;
          CREATE TRIGGER IF NOT EXISTS immutable_events_delete BEFORE DELETE ON events BEGIN SELECT RAISE(ABORT,'immutable audit'); END;
          CREATE TRIGGER IF NOT EXISTS immutable_provenance_update BEFORE UPDATE ON provenance BEGIN SELECT RAISE(ABORT,'immutable provenance'); END;
          CREATE TRIGGER IF NOT EXISTS immutable_provenance_delete BEFORE DELETE ON provenance BEGIN SELECT RAISE(ABORT,'immutable provenance'); END;").map_err(db_error)?;
        store.verify()?;
        Ok(store)
    }
    pub fn verify(&self) -> Result<Value> {
        let mut previous = "0".repeat(64);
        let mut count = 0;
        let mut statement = self
            .db
            .prepare("SELECT body,digest FROM events ORDER BY seq")
            .map_err(db_error)?;
        let rows = statement
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .map_err(db_error)?;
        for row in rows {
            let (body, digest) = row.map_err(db_error)?;
            let body: Value = serde_json::from_str(&body).map_err(|_| "AUDIT_CHAIN_INVALID")?;
            ensure(
                body["previousHash"] == previous && hash(&body) == digest,
                "AUDIT_CHAIN_INVALID",
            )?;
            if let Some(id) = body["provenanceId"].as_str() {
                let raw: Option<String> = self
                    .db
                    .query_row("SELECT body FROM provenance WHERE id=?", [id], |r| r.get(0))
                    .optional()
                    .map_err(db_error)?;
                let node: Value = serde_json::from_str(&raw.ok_or("PROVENANCE_INVALID")?)
                    .map_err(|_| "PROVENANCE_INVALID")?;
                ensure(hash(&node) == body["provenanceHash"], "PROVENANCE_INVALID")?;
            }
            previous = digest;
            count += 1;
        }
        Ok(json!({"count":count,"head":previous}))
    }
    fn cached(&self, id: &str) -> Result<Option<(String, Value)>> {
        let row: Option<(String, String)> = self
            .db
            .query_row(
                "SELECT digest,response FROM requests WHERE id=?",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(db_error)?;
        row.map(|(d, r)| {
            serde_json::from_str(&r)
                .map(|r| (d, r))
                .map_err(|_| "PERSISTENCE_FAILURE".into())
        })
        .transpose()
    }
    pub fn frozen(&self, id: &str) -> Result<bool> {
        Ok(self
            .db
            .query_row("SELECT frozen FROM graphs WHERE id=?", [id], |r| {
                r.get::<_, i32>(0)
            })
            .optional()
            .map_err(db_error)?
            == Some(1))
    }
    fn commit(
        &self,
        req: &Value,
        digest: &str,
        response: &Value,
        nodes: &[Value],
        events: &[Value],
    ) -> Result<()> {
        self.db.execute_batch("BEGIN IMMEDIATE").map_err(db_error)?;
        let result = (|| {
            let verified = self.verify()?;
            ensure(
                self.cached(req["requestId"].as_str().unwrap())?.is_none(),
                "REQUEST_REPLAY",
            )?;
            ensure(
                !self.frozen(req["graphId"].as_str().unwrap())? || response["status"] == "HALTED",
                "GRAPH_FROZEN",
            )?;
            let mut previous = verified["head"].as_str().unwrap().to_owned();
            for node in nodes {
                self.db
                    .execute(
                        "INSERT INTO provenance VALUES (?,?)",
                        params![node["id"].as_str().unwrap(), node.to_string()],
                    )
                    .map_err(db_error)?;
            }
            for event in events {
                let mut body = event.clone();
                body["previousHash"] = json!(previous);
                previous = hash(&body);
                self.db
                    .execute(
                        "INSERT INTO events(body,digest) VALUES (?,?)",
                        params![body.to_string(), previous],
                    )
                    .map_err(db_error)?;
            }
            self.db.execute("INSERT INTO graphs VALUES (?,?) ON CONFLICT(id) DO UPDATE SET frozen=MAX(frozen,excluded.frozen)",params![req["graphId"].as_str().unwrap(),i32::from(response["status"] != "COMPLETED")]).map_err(db_error)?;
            self.db
                .execute(
                    "INSERT INTO requests VALUES (?,?,?)",
                    params![
                        req["requestId"].as_str().unwrap(),
                        digest,
                        response.to_string()
                    ],
                )
                .map_err(db_error)?;
            self.db.execute_batch("COMMIT").map_err(db_error)
        })();
        if result.is_err() {
            let _ = self.db.execute_batch("ROLLBACK");
        }
        result
    }
}
fn timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}
pub fn new_id() -> String {
    Uuid::new_v4().to_string()
}
pub fn execute(store: &Store, principals: &Value, req: &Value) -> Value {
    let permit = match david_execution_gate::authorize("corporate.validation") {
        Ok(permit) => permit,
        Err(code) => {
            return json!({"status":"HALTED","errorCode":code,"decisionSupportOnly":true,"paymentExecuted":false,"migrationApproved":false});
        }
    };
    execute_inner(store, principals, req, Some(&permit))
}
fn execute_inner(
    store: &Store,
    principals: &Value,
    req: &Value,
    permit: Option<&david_execution_gate::Permit>,
) -> Value {
    let mut digest = None;
    let mut nodes = Vec::new();
    let mut events = Vec::new();
    let mut selected = Vec::new();
    let mut output = Value::Null;
    let mut replay = false;
    let outcome: Result<Option<Value>> = (|| {
        if let Some(permit) = permit {
            permit
                .require("corporate.validation", david_execution_gate::now()?)
                .map_err(str::to_owned)?;
        }
        admit(req)?;
        digest = Some(hash(req));
        store.verify()?;
        let cached = store.cached(req["requestId"].as_str().unwrap())?;
        replay = cached.is_some();
        if let Some((d, _)) = &cached {
            ensure(
                d == digest.as_ref().unwrap().as_str(),
                "REQUEST_ID_CONFLICT",
            )?;
        }
        let principal = principals
            .get(req["requestor"].as_str().unwrap())
            .ok_or("UNKNOWN_REQUESTOR")?;
        object(principal, &["agents", "permissions"])?;
        let capability = match req["type"].as_str().unwrap() {
            "LEDGER-VALIDATE" => "ledger.validate",
            "DOCUMENT-INSPECT" => "document.inspect",
            "COBOL-INSPECT" => "legacy.cobol.inspect",
            "MIGRATION-VALIDATE" => "migration.validate",
            _ => return Err("UNKNOWN_TOOL".into()),
        };
        selected = select_minimum(
            &[capability],
            &principal["agents"],
            &principal["permissions"],
        )?;
        if let Some((_, response)) = cached {
            return Ok(Some(response));
        }
        ensure(
            !store.frozen(req["graphId"].as_str().unwrap())?,
            "GRAPH_FROZEN",
        )?;
        for agent in &selected {
            let value = validate(agent, &req["payload"])?;
            let time = timestamp();
            let tool = role(agent).unwrap().2;
            let mut sources = vec![req["payload"]["source"].clone()];
            if let Some(s) = req["payload"].get("modernSource") {
                sources.push(s.clone());
            }
            let node = json!({"id":new_id(),"result":"OK","evidence":value,"errorCode":0,"errorMessage":"","sources":sources,"agentId":agent,"toolId":tool,"timestamp":time,"inputHash":hash(&req["payload"]),"outputHash":hash(&value),"transformation":tool,"parents":nodes.iter().map(|n: &Value| n["id"].clone()).collect::<Vec<_>>(),"ruleChain":["PRINCIPAL-ALLOWLIST","MINIMUM-CAPABILITY-COVER","TYPED-PAYLOAD","EXACT-VALIDATION"],"assumptions":["Caller-supplied evidence; source authenticity requires an external trusted ingest boundary."],"uncertainty":0,"uncertaintyScope":"deterministic transformation only","riskSignal":0});
            events.push(json!({"eventId":new_id(),"requestId":req["requestId"],"traceId":req["traceId"],"graphId":req["graphId"],"agentId":agent,"toolId":tool,"inputHash":node["inputHash"],"outputHash":node["outputHash"],"timestamp":time,"result":"OK","riskSignal":0,"provenanceId":node["id"],"provenanceHash":hash(&node)}));
            nodes.push(node);
            output = value;
        }
        if let Some(permit) = permit {
            permit
                .require("corporate.validation", david_execution_gate::now()?)
                .map_err(str::to_owned)?;
        }
        Ok(None)
    })();
    if let Ok(Some(cached)) = outcome {
        return cached;
    }
    let code = outcome.err();
    let status = match code.as_deref() {
        None => "COMPLETED",
        Some("LEDGER_MISMATCH" | "MIGRATION_DIVERGENCE") => "ESCALATED",
        _ => "HALTED",
    };
    let mut response = json!({"status":status,"agents":selected,"provenance":nodes,"decisionSupportOnly":true,"executionAuthority":"RUST-VALIDATION-HARNESS","paymentExecuted":false,"migrationApproved":false});
    if let Some(permit) = permit {
        response["executionEntitlement"] = permit.summary();
    }
    if let Some(c) = &code {
        response["errorCode"] = json!(c);
    } else {
        response["output"] = output;
    }
    if replay
        || digest.is_none()
        || matches!(
            code.as_deref(),
            Some("REQUEST_ID_CONFLICT" | "AUDIT_CHAIN_INVALID" | "PROVENANCE_INVALID")
        )
    {
        return response;
    }
    events.push(json!({"eventId":new_id(),"requestId":req["requestId"],"traceId":req["traceId"],"graphId":req["graphId"],"agentId":"SUPERVISOR","toolId":"REQUEST-FINALIZE","timestamp":timestamp(),"result":status,"inputHash":digest,"outputHash":hash(&response),"riskSignal":if status=="COMPLETED" {0} else {1},"errorCode":code}));
    if store
        .commit(req, digest.as_ref().unwrap(), &response, &nodes, &events)
        .is_err()
    {
        return json!({"status":"HALTED","errorCode":"PERSISTENCE_FAILURE","decisionSupportOnly":true,"paymentExecuted":false,"migrationApproved":false});
    }
    response
}
pub fn demo_request() -> Value {
    let text = "Synthetic David development fixture.\nDocument inspection only.";
    json!({"requestId":new_id(),"traceId":new_id(),"graphId":new_id(),"requestor":"demo-reviewer","type":"DOCUMENT-INSPECT","payload":{"source":{"id":"DEVELOPMENT-FIXTURE","location":"rust/control-plane","sha256":hash(&json!(text))},"text":text}})
}
pub fn demo_policy() -> Value {
    json!({"demo-reviewer":{"agents":["DOCUMENT"],"permissions":["document:read"]}})
}

#[cfg(test)]
#[path = "../tests/controls.rs"]
mod controls;
