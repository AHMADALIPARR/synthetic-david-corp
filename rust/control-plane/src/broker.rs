//! SDABI001: GnuCOBOL ASCII / C ABI. All fields are bytes, never native integers.
use crate::{Result, Store, admit, canonical, ensure, hash, id, role};
use serde_json::{Value, json};
use std::{io::Read, path::Path};

pub const ABI: &[u8; 8] = b"SDABI001";
pub const COMMAND: &str = "EXECUTE-AND-COMMIT";
#[repr(C)]
#[derive(Clone, Copy)]
pub struct NativeRequest {
    pub abi: [u8; 8],
    pub request_id: [u8; 36],
    pub trace_id: [u8; 36],
    pub graph_id: [u8; 36],
    pub requestor: [u8; 64],
    pub request_type: [u8; 32],
    pub payload_length: [u8; 8],
    pub payload: [u8; 8192],
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct NativeResult {
    pub abi: [u8; 8],
    pub status: [u8; 16],
    pub evidence: [u8; 2048],
    pub provenance_id: [u8; 36],
    pub agent_id: [u8; 16],
    pub tool_id: [u8; 32],
    pub input_hash: [u8; 64],
    pub output_hash: [u8; 64],
    pub timestamp: [u8; 27],
    pub uncertainty: [u8; 4],
    pub risk: [u8; 4],
    pub error_code: [u8; 4],
    pub error_message: [u8; 256],
}
pub const REQUEST_BYTES: usize = 8412;
pub const RESULT_BYTES: usize = 2579;
const _: () = assert!(std::mem::size_of::<NativeRequest>() == REQUEST_BYTES);
const _: () = assert!(std::mem::size_of::<NativeResult>() == RESULT_BYTES);
const _: () = assert!(std::mem::align_of::<NativeRequest>() == 1);
const _: () = assert!(std::mem::align_of::<NativeResult>() == 1);
fn field<const N: usize>(text: &str) -> Result<[u8; N]> {
    ensure(
        text.len() <= N && text.is_ascii() && !text.bytes().any(|b| b == 0 || b.is_ascii_control()),
        "ABI_FIELD_INVALID",
    )?;
    let mut bytes = [b' '; N];
    bytes[..text.len()].copy_from_slice(text.as_bytes());
    Ok(bytes)
}
fn string(bytes: &[u8]) -> Result<&str> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| "ABI_ENCODING_INVALID")?
        .trim_end_matches(' ');
    ensure(
        text.is_ascii() && !text.bytes().any(|b| b == 0 || b.is_ascii_control()),
        "ABI_FIELD_INVALID",
    )?;
    Ok(text)
}
fn digits(bytes: &[u8]) -> Result<usize> {
    ensure(bytes.iter().all(u8::is_ascii_digit), "ABI_NUMERIC_INVALID")?;
    std::str::from_utf8(bytes)
        .unwrap()
        .parse()
        .map_err(|_| "ABI_NUMERIC_INVALID".into())
}
impl NativeRequest {
    pub fn encode(request: &Value) -> Result<Self> {
        admit(request)?;
        for key in ["requestId", "traceId", "graphId", "requestor", "type"] {
            let text = request[key].as_str().unwrap();
            ensure(text.trim() == text, "ABI_FIELD_INVALID")?;
        }
        let payload = canonical(&request["payload"]);
        let mut bytes = [b' '; 8192];
        bytes[..payload.len()].copy_from_slice(payload.as_bytes());
        Ok(Self {
            abi: *ABI,
            request_id: field(request["requestId"].as_str().unwrap())?,
            trace_id: field(request["traceId"].as_str().unwrap())?,
            graph_id: field(request["graphId"].as_str().unwrap())?,
            requestor: field(request["requestor"].as_str().unwrap())?,
            request_type: field(request["type"].as_str().unwrap())?,
            payload_length: field(&format!("{:08}", payload.len()))?,
            payload: bytes,
        })
    }
    fn decode(&self) -> Result<Value> {
        ensure(&self.abi == ABI, "ABI_VERSION_UNSUPPORTED")?;
        let len = digits(&self.payload_length)?;
        ensure((1..=8192).contains(&len), "ABI_PAYLOAD_LENGTH_INVALID")?;
        ensure(
            self.payload[len..].iter().all(|b| *b == b' '),
            "ABI_PAYLOAD_PADDING_INVALID",
        )?;
        let payload: Value =
            serde_json::from_slice(&self.payload[..len]).map_err(|_| "ABI_PAYLOAD_JSON_INVALID")?;
        let request = json!({"requestId":string(&self.request_id)?,"traceId":string(&self.trace_id)?,"graphId":string(&self.graph_id)?,"requestor":string(&self.requestor)?,"type":string(&self.request_type)?,"payload":payload});
        admit(&request)?;
        Ok(request)
    }
    pub fn bytes(&self) -> &[u8] {
        // repr(C), alignment one, all fields are initialized byte arrays.
        unsafe { std::slice::from_raw_parts(self as *const Self as *const u8, REQUEST_BYTES) }
    }
}
impl NativeResult {
    fn empty() -> Self {
        Self {
            abi: *ABI,
            status: [b' '; 16],
            evidence: [b' '; 2048],
            provenance_id: [b' '; 36],
            agent_id: [b' '; 16],
            tool_id: [b' '; 32],
            input_hash: [b' '; 64],
            output_hash: [b' '; 64],
            timestamp: [b' '; 27],
            uncertainty: *b"0000",
            risk: *b"0000",
            error_code: *b"0000",
            error_message: [b' '; 256],
        }
    }
    fn failure(code: &str) -> Self {
        let mut out = Self::empty();
        out.status = field("HALTED").unwrap();
        out.error_code = *b"0016";
        out.error_message =
            field(code).unwrap_or_else(|_| field("BROKER_INTERNAL_FAILURE").unwrap());
        out
    }
    fn encode(response: &Value, request_id: &str) -> Result<Self> {
        let status = response["status"].as_str().ok_or("ABI_RESULT_INVALID")?;
        if status == "HALTED" {
            return Ok(Self::failure(
                response["errorCode"].as_str().unwrap_or("BROKER_FAILED"),
            ));
        }
        let mut out = Self::empty();
        out.status = field(status)?;
        if status == "ESCALATED" {
            out.risk = *b"0001";
            out.error_code = *b"0008";
            out.error_message = field(response["errorCode"].as_str().ok_or("ABI_RESULT_INVALID")?)?;
            // Failure evidence is the durable final-request audit, not a fabricated provenance node.
            out.evidence = field(&canonical(
                &json!({"kind":"AUDITED-REQUEST-V1","requestId":request_id,"responseHash":hash(response),"result":"ESCALATED"}),
            ))?;
            return Ok(out);
        }
        ensure(status == "COMPLETED", "ABI_RESULT_INVALID")?;
        let nodes = response["provenance"]
            .as_array()
            .ok_or("ABI_RESULT_INVALID")?;
        ensure(nodes.len() == 1, "ABI_RESULT_INVALID")?;
        let node = &nodes[0];
        id(&node["id"])?;
        let agent = node["agentId"].as_str().ok_or("ABI_RESULT_INVALID")?;
        let (_, _, tool) = role(agent).ok_or("ABI_RESULT_INVALID")?;
        ensure(
            node["toolId"] == tool
                && node["result"] == "OK"
                && node["errorCode"] == 0
                && node["riskSignal"] == 0
                && node["uncertainty"] == 0
                && response["agents"] == json!([agent]),
            "ABI_RESULT_INVALID",
        )?;
        ensure(
            hash(&node["evidence"]) == node["outputHash"]
                && hash(&response["output"]) == node["outputHash"],
            "ABI_RESULT_INVALID",
        )?;
        for k in ["inputHash", "outputHash"] {
            let value = node[k].as_str().ok_or("ABI_RESULT_INVALID")?;
            ensure(
                value.len() == 64
                    && value
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
                "ABI_RESULT_INVALID",
            )?;
        }
        out.provenance_id = field(node["id"].as_str().unwrap())?;
        out.agent_id = field(agent)?;
        out.tool_id = field(tool)?;
        out.input_hash = field(node["inputHash"].as_str().unwrap())?;
        out.output_hash = field(node["outputHash"].as_str().unwrap())?;
        let timestamp = node["timestamp"].as_str().ok_or("ABI_RESULT_INVALID")?;
        chrono::DateTime::parse_from_rfc3339(timestamp).map_err(|_| "ABI_RESULT_INVALID")?;
        out.timestamp = field(timestamp)?;
        out.evidence = field(&canonical(
            &json!({"kind":"SQLITE-PROVENANCE-V1","requestId":request_id,"provenanceId":node["id"],"outputHash":node["outputHash"]}),
        ))?;
        Ok(out)
    }
    pub fn bytes(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self as *const Self as *const u8, RESULT_BYTES) }
    }
    pub fn decode_bytes(bytes: &[u8]) -> Result<Value> {
        ensure(bytes.len() == RESULT_BYTES, "ABI_RESULT_LENGTH_INVALID")?;
        // Alignment is one, and every byte pattern is valid for this byte-array struct.
        let record = unsafe { std::ptr::read_unaligned(bytes.as_ptr() as *const Self) };
        ensure(&record.abi == ABI, "ABI_VERSION_UNSUPPORTED")?;
        let evidence = string(&record.evidence)?;
        Ok(
            json!({"abi":"SDABI001","status":string(&record.status)?,"evidence":if evidence.is_empty() {Value::Null} else {serde_json::from_str(evidence).map_err(|_|"ABI_RESULT_INVALID")?},"provenanceId":string(&record.provenance_id)?,"agentId":string(&record.agent_id)?,"toolId":string(&record.tool_id)?,"inputHash":string(&record.input_hash)?,"outputHash":string(&record.output_hash)?,"timestamp":string(&record.timestamp)?,"uncertaintyUnits":digits(&record.uncertainty)?,"riskSignal":digits(&record.risk)?,"errorCode":digits(&record.error_code)?,"errorMessage":string(&record.error_message)?}),
        )
    }
}
fn validate_boundary(response: &Value) -> Result<()> {
    NativeResult::encode(response, "00000000-0000-0000-0000-000000000000").map(|_| ())
}
fn read_policy(path: &Path) -> Result<Value> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|_| "BROKER_POLICY_UNAVAILABLE")?
        .take(262145)
        .read_to_end(&mut bytes)
        .map_err(|_| "BROKER_POLICY_UNAVAILABLE")?;
    ensure(bytes.len() <= 262144, "BROKER_POLICY_TOO_LARGE")?;
    let policy: Value = serde_json::from_slice(&bytes).map_err(|_| "BROKER_POLICY_INVALID")?;
    ensure(policy.is_object(), "BROKER_POLICY_INVALID")?;
    Ok(policy)
}
fn dispatch(command: &[u8; 64], request: &NativeRequest) -> Result<NativeResult> {
    ensure(string(command)? == COMMAND, "BROKER_UNKNOWN_COMMAND")?;
    ensure(&request.abi == ABI, "ABI_VERSION_UNSUPPORTED")?;
    let permit = david_execution_gate::authorize("corporate.validation").map_err(str::to_owned)?;
    let request = request.decode()?;
    let policy_path =
        std::env::var_os("DAVID_PRINCIPALS_FILE").ok_or("BROKER_POLICY_UNCONFIGURED")?;
    let db_path = std::env::var("DAVID_AUDIT_DB").map_err(|_| "BROKER_STORE_UNCONFIGURED")?;
    ensure(
        db_path != ":memory:" && !db_path.trim().is_empty(),
        "BROKER_DURABLE_STORE_REQUIRED",
    )?;
    let policy = read_policy(Path::new(&policy_path))?;
    let store = Store::open(&db_path)?;
    let response = crate::execute_checked(
        &store,
        &policy,
        &request,
        Some(&permit),
        Some(validate_boundary),
    );
    NativeResult::encode(&response, request["requestId"].as_str().unwrap())
}

/// GnuCOBOL BY REFERENCE entry point, statically resolved with -K SD_BROKER.
///
/// # Safety
/// Non-null pointers must address nonoverlapping live arrays/records of the EXACT
/// SDABI001 sizes (command64, request8412, result2579, rc4). The result and rc must
/// be writable. Caller ownership/data races and invalid pointers cannot be checked.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn SD_BROKER(
    command: *const [u8; 64],
    request: *const NativeRequest,
    result: *mut NativeResult,
    rc: *mut [u8; 4],
) -> i32 {
    if command.is_null() || request.is_null() || result.is_null() || rc.is_null() {
        return 16;
    }
    unsafe {
        std::ptr::write(rc, *b"0016");
        std::ptr::write(result, NativeResult::failure("BROKER_INTERNAL_FAILURE"));
    }
    let outcome = std::panic::catch_unwind(|| {
        let command = unsafe { std::ptr::read(command) };
        let request = unsafe { std::ptr::read(request) };
        dispatch(&command, &request)
    });
    let output = match outcome {
        Ok(Ok(result)) => result,
        Ok(Err(code)) => NativeResult::failure(&code),
        Err(_) => NativeResult::failure("BROKER_INTERNAL_FAILURE"),
    };
    let success =
        output.status.starts_with(b"COMPLETED") || output.status.starts_with(b"ESCALATED");
    unsafe {
        std::ptr::write(result, output);
        std::ptr::write(rc, if success { *b"0000" } else { *b"0016" });
    }
    if success { 0 } else { 16 }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_layout_roundtrip_and_numeric_failures() {
        let request = crate::demo_request();
        let mut native = NativeRequest::encode(&request).unwrap();
        assert_eq!(native.bytes().len(), REQUEST_BYTES);
        assert_eq!(native.decode().unwrap(), request);
        native.payload_length = *b"0000000x";
        assert_eq!(native.decode().unwrap_err(), "ABI_NUMERIC_INVALID");
        native.payload_length = *b"00008193";
        assert_eq!(native.decode().unwrap_err(), "ABI_PAYLOAD_LENGTH_INVALID");
        native.abi = *b"SDABI002";
        assert_eq!(native.decode().unwrap_err(), "ABI_VERSION_UNSUPPORTED");
    }
    #[test]
    fn entry_rejects_unknown_command_and_unlicensed_execution() {
        let request = NativeRequest::encode(&crate::demo_request()).unwrap();
        let mut result = NativeResult::empty();
        let mut rc = *b"0000";
        let command = field("PAYMENT-ORDER").unwrap();
        assert_eq!(
            unsafe { SD_BROKER(&command, &request, &mut result, &mut rc) },
            16
        );
        assert_eq!(
            string(&result.error_message).unwrap(),
            "BROKER_UNKNOWN_COMMAND"
        );
        assert_eq!(&rc, b"0016");
        if david_execution_gate::issuer_root().is_err() {
            let command = field(COMMAND).unwrap();
            unsafe { SD_BROKER(&command, &request, &mut result, &mut rc) };
            assert_eq!(
                string(&result.error_message).unwrap(),
                "LICENSE_ISSUER_UNCONFIGURED"
            );
        }
        assert_eq!(
            unsafe { SD_BROKER(std::ptr::null(), &request, &mut result, &mut rc) },
            16
        );
    }
    #[test]
    fn evidence_is_a_durable_reference_not_truncated_output() {
        let store = Store::open(":memory:").unwrap();
        let request = crate::demo_request();
        let response = crate::execute_checked(
            &store,
            &crate::demo_policy(),
            &request,
            None,
            Some(validate_boundary),
        );
        let native =
            NativeResult::encode(&response, request["requestId"].as_str().unwrap()).unwrap();
        let decoded = NativeResult::decode_bytes(native.bytes()).unwrap();
        assert_eq!(decoded["status"], "COMPLETED");
        assert_eq!(decoded["evidence"]["kind"], "SQLITE-PROVENANCE-V1");
        assert_eq!(decoded["outputHash"], hash(&response["output"]));
        assert_eq!(store.verify().unwrap()["count"], 2);
    }
    #[test]
    fn boundary_failure_rolls_back_success_and_cache_tampering_is_denied() {
        fn reject(_: &Value) -> Result<()> {
            Err("ABI_RESULT_INVALID".into())
        }
        let store = Store::open(":memory:").unwrap();
        let request = crate::demo_request();
        let response =
            crate::execute_checked(&store, &crate::demo_policy(), &request, None, Some(reject));
        assert_eq!(response["status"], "HALTED");
        assert_eq!(response["errorCode"], "ABI_RESULT_INVALID");
        assert_eq!(store.verify().unwrap()["count"], 1);
        assert!(store.frozen(request["graphId"].as_str().unwrap()).unwrap());
        assert_eq!(
            store
                .db
                .query_row("SELECT COUNT(*) FROM provenance", [], |r| r
                    .get::<_, i32>(0))
                .unwrap(),
            0
        );
        let store = Store::open(":memory:").unwrap();
        let request = crate::demo_request();
        crate::execute_inner(&store, &crate::demo_policy(), &request, None);
        store
            .db
            .execute("UPDATE requests SET response='{}'", [])
            .unwrap();
        let response = crate::execute_inner(&store, &crate::demo_policy(), &request, None);
        assert_eq!(response["errorCode"], "CACHE_INTEGRITY_INVALID");
        assert_eq!(store.verify().unwrap()["count"], 2);
    }
}

#[cfg(all(test, target_os = "linux"))]
#[path = "native_integration.rs"]
mod native_integration;
