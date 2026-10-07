use david_qwen_endpoint::{
    LISTEN, MAX_BODY, MODEL, UPSTREAM, prepare, restore, restore_namespaces, sse,
};
use reqwest::blocking::Client;
use serde_json::{Value, json};
use std::io::Read;
use std::time::Duration;
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

fn send(request: Request, status: u16, mime: &str, body: String) {
    let response = Response::from_string(body)
        .with_status_code(StatusCode(status))
        .with_header(Header::from_bytes("Content-Type", mime).expect("constant header"))
        .with_header(Header::from_bytes("Cache-Control", "no-store").expect("constant header"));
    let _ = request.respond(response);
}
fn error(request: Request, status: u16, code: &str) {
    send(
        request,
        status,
        "application/json",
        json!({"error":{"type":"local_adapter_error","code":code,"message":code}}).to_string(),
    );
}
fn infer(client: &Client, prepared: &david_qwen_endpoint::Prepared) -> Result<Value, &'static str> {
    let mut request = client.post(UPSTREAM).json(&prepared.body);
    if let Ok(key) = std::env::var("LM_STUDIO_API_KEY") {
        request = request.bearer_auth(key);
    }
    let response = request.send().map_err(|_| "UPSTREAM_UNAVAILABLE")?;
    if !response.status().is_success() {
        return Err("UPSTREAM_REJECTED_REQUEST");
    }
    let mut bytes = Vec::new();
    response
        .take(2_097_153)
        .read_to_end(&mut bytes)
        .map_err(|_| "UPSTREAM_READ_FAILED")?;
    if bytes.len() > 2_097_152 {
        return Err("UPSTREAM_RESPONSE_TOO_LARGE");
    }
    let response: Value = serde_json::from_slice(&bytes).map_err(|_| "UPSTREAM_INVALID_JSON")?;
    let mut response = restore(response, &prepared.custom_tools)?;
    restore_namespaces(&mut response, &prepared.namespaces);
    Ok(response)
}
fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let server = Server::http(LISTEN)?;
    let client = Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(90))
        .build()?;
    eprintln!("Qwen Codex Responses adapter: http://{LISTEN}/v1 -> local LM Studio (buffered SSE)");
    for mut request in server.incoming_requests() {
        if request.headers().iter().any(|h| h.field.equiv("Origin")) {
            error(request, 403, "BROWSER_ORIGIN_REJECTED");
            continue;
        }
        match (request.method(), request.url()) {
            (&Method::Get, "/health") => {
                send(request,200,"application/json",json!({"service":"david-qwen-endpoint","pid":std::process::id(),"model":MODEL,"upstreamVerified":false,"bufferedSse":true}).to_string());
                continue;
            }
            (&Method::Get, "/v1/models") => {
                send(request,200,"application/json",json!({"object":"list","data":[{"id":MODEL,"object":"model","owned_by":"local"}]}).to_string());
                continue;
            }
            (&Method::Post, "/v1/responses") => {}
            _ => {
                error(request, 404, "UNKNOWN_ENDPOINT");
                continue;
            }
        }
        if request
            .headers()
            .iter()
            .any(|h| h.field.equiv("Content-Encoding") && h.value.as_str() != "identity")
        {
            error(request, 415, "COMPRESSION_UNSUPPORTED");
            continue;
        }
        if !request.headers().iter().any(|h| {
            h.field.equiv("Content-Type") && h.value.as_str().starts_with("application/json")
        }) {
            error(request, 415, "JSON_REQUIRED");
            continue;
        }
        let mut bytes = Vec::new();
        if request
            .as_reader()
            .take((MAX_BODY + 1) as u64)
            .read_to_end(&mut bytes)
            .is_err()
        {
            error(request, 400, "INVALID_BODY");
            continue;
        }
        if bytes.len() > MAX_BODY {
            error(request, 413, "BODY_TOO_LARGE");
            continue;
        }
        let body = match serde_json::from_slice(&bytes) {
            Ok(v) => v,
            Err(_) => {
                error(request, 400, "INVALID_JSON");
                continue;
            }
        };
        let prepared = match prepare(body) {
            Ok(v) => v,
            Err(code) => {
                error(request, 400, code);
                continue;
            }
        };
        match infer(&client, &prepared) {
            Ok(response) if prepared.streaming => match sse(&response) {
                Ok(events) => send(request, 200, "text/event-stream", events),
                Err(code) => error(request, 502, code),
            },
            Ok(response) => send(request, 200, "application/json", response.to_string()),
            Err(code) => error(request, 502, code),
        }
    }
    Ok(())
}
