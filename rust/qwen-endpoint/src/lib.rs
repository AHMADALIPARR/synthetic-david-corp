//! Codex Responses wire adapter for a fixed local Qwen/LM Studio provider.
//! This crate translates protocols. It never executes tools or authorizes money.
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub const MODEL: &str = "qwen-local";
pub const UPSTREAM: &str = "http://127.0.0.1:1234/v1/responses";
pub const LISTEN: &str = "127.0.0.1:1235";
pub const MAX_BODY: usize = 524_288;

pub struct Prepared {
    pub body: Value,
    pub custom_tools: BTreeSet<String>,
    pub streaming: bool,
    pub namespaces: BTreeMap<String, (String, String)>,
}

fn array_content(input: &Value) -> Result<(), &'static str> {
    if input.is_string() {
        return Ok(());
    }
    for item in input.as_array().ok_or("INVALID_INPUT")? {
        match item
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("message")
        {
            "message" => {
                let role = item
                    .get("role")
                    .and_then(Value::as_str)
                    .ok_or("INVALID_ROLE")?;
                if !["system", "developer", "user", "assistant"].contains(&role) {
                    return Err("INVALID_ROLE");
                }
                let content = item.get("content").ok_or("INVALID_CONTENT")?;
                if content.is_string() {
                    continue;
                }
                for part in content.as_array().ok_or("INVALID_CONTENT")? {
                    if !["input_text", "output_text"]
                        .contains(&part.get("type").and_then(Value::as_str).unwrap_or(""))
                    {
                        return Err("UNSUPPORTED_CONTENT");
                    }
                    if !part.get("text").is_some_and(Value::is_string) {
                        return Err("INVALID_CONTENT");
                    }
                }
            }
            "function_call" | "custom_tool_call" => {
                for key in ["call_id", "name"] {
                    if !item.get(key).is_some_and(Value::is_string) {
                        return Err("INVALID_TOOL_ITEM");
                    }
                }
                let field = if item["type"] == "custom_tool_call" {
                    "input"
                } else {
                    "arguments"
                };
                if !item.get(field).is_some_and(Value::is_string) {
                    return Err("INVALID_TOOL_ITEM");
                }
            }
            "function_call_output" | "custom_tool_call_output" => {
                if !item.get("call_id").is_some_and(Value::is_string)
                    || !item.get("output").is_some_and(Value::is_string)
                {
                    return Err("INVALID_TOOL_ITEM");
                }
            }
            _ => return Err("UNSUPPORTED_INPUT_ITEM"),
        }
    }
    Ok(())
}

pub fn prepare(mut body: Value) -> Result<Prepared, &'static str> {
    let obj = body.as_object().ok_or("INVALID_REQUEST")?;
    if obj.get("model").and_then(Value::as_str) != Some(MODEL) {
        return Err("MODEL_NOT_ALLOWED");
    }
    if obj
        .get("previous_response_id")
        .is_some_and(|v| !v.is_null())
    {
        return Err("STATEFUL_REQUEST_UNSUPPORTED");
    }
    if obj.get("store") == Some(&Value::Bool(true))
        || obj.get("background") == Some(&Value::Bool(true))
    {
        return Err("STATEFUL_REQUEST_UNSUPPORTED");
    }
    if obj.get("stream").is_some_and(|v| !v.is_boolean()) {
        return Err("INVALID_STREAM");
    }
    if let Some(v) = obj.get("max_output_tokens") {
        if !v.as_u64().is_some_and(|v| (1..=512).contains(&v)) {
            return Err("OUTPUT_BUDGET_EXCEEDED");
        }
    }
    array_content(body.get("input").ok_or("INVALID_INPUT")?)?;
    let streaming = body["stream"].as_bool().unwrap_or(false);
    let mut namespaces = BTreeMap::new();
    if let Some(tools) = body.get_mut("tools") {
        let original = tools.as_array().ok_or("INVALID_TOOLS")?.clone();
        let mut flat = Vec::new();
        for tool in original {
            if tool["type"] == "namespace" {
                let ns = tool["name"].as_str().ok_or("INVALID_NAMESPACE")?;
                if ns.is_empty() || !ns.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
                    return Err("INVALID_NAMESPACE");
                }
                for nested in tool["tools"].as_array().ok_or("INVALID_NAMESPACE")? {
                    if !["function", "custom"].contains(&nested["type"].as_str().unwrap_or("")) {
                        return Err("UNSUPPORTED_TOOL_TYPE");
                    }
                    let name = nested["name"].as_str().ok_or("INVALID_TOOL_NAME")?;
                    let wire_name = format!("{ns}__{name}");
                    if namespaces
                        .insert(wire_name.clone(), (ns.to_owned(), name.to_owned()))
                        .is_some()
                    {
                        return Err("INVALID_TOOL_NAME");
                    }
                    let mut nested = nested.clone();
                    nested["name"] = json!(wire_name);
                    flat.push(nested);
                }
            } else {
                flat.push(tool);
            }
        }
        *tools = json!(flat);
    }
    let mut custom_tools = BTreeSet::new();
    let mut names = BTreeSet::new();
    if let Some(tools) = body.get_mut("tools") {
        for tool in tools.as_array_mut().ok_or("INVALID_TOOLS")? {
            let name = tool
                .get("name")
                .and_then(Value::as_str)
                .ok_or("INVALID_TOOL_NAME")?
                .to_owned();
            if name.is_empty() || !names.insert(name.clone()) {
                return Err("INVALID_TOOL_NAME");
            }
            match tool.get("type").and_then(Value::as_str) {
                Some("function") => {
                    if !tool.get("parameters").is_some_and(Value::is_object) {
                        return Err("INVALID_TOOL_SCHEMA");
                    }
                }
                Some("custom") => {
                    custom_tools.insert(name.clone());
                    let description = tool
                        .get("description")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_owned();
                    *tool = json!({"type":"function", "name":name,
                        "description":format!("{description}\nReturn the original tool input as a string in the input field."),
                        "parameters":{"type":"object","properties":{"input":{"type":"string"}},"required":["input"],"additionalProperties":false}});
                }
                _ => return Err("UNSUPPORTED_TOOL_TYPE"),
            }
        }
    }
    if let Some(items) = body.get_mut("input").and_then(Value::as_array_mut) {
        for item in items {
            if ["function_call", "custom_tool_call"].contains(&item["type"].as_str().unwrap_or(""))
            {
                if let Some(ns) = item.get("namespace").and_then(Value::as_str) {
                    let name = item["name"].as_str().ok_or("INVALID_TOOL_NAME")?;
                    let wire = format!("{ns}__{name}");
                    if !namespaces.contains_key(&wire) {
                        return Err("UNKNOWN_NAMESPACED_TOOL");
                    }
                    item["name"] = json!(wire);
                }
            }
            if let Some(obj) = item.as_object_mut() {
                obj.remove("namespace");
            }
            if item["type"] == "custom_tool_call" {
                let original = item.get("input").cloned().ok_or("INVALID_TOOL_ITEM")?;
                item["type"] = json!("function_call");
                item["arguments"] = json!(json!({"input":original}).to_string());
                item.as_object_mut()
                    .ok_or("INVALID_TOOL_ITEM")?
                    .remove("input");
            } else if item["type"] == "custom_tool_call_output" {
                item["type"] = json!("function_call_output");
            }
        }
    }
    if let Some(choice) = body.get("tool_choice") {
        if choice.is_object() {
            if !["custom", "function"].contains(&choice["type"].as_str().unwrap_or("")) {
                return Err("UNSUPPORTED_TOOL_CHOICE");
            }
            let bare = choice["name"].as_str().ok_or("INVALID_TOOL_NAME")?;
            let name = choice
                .get("namespace")
                .and_then(Value::as_str)
                .map(|ns| format!("{ns}__{bare}"))
                .unwrap_or_else(|| bare.to_owned());
            if !names.contains(&name) {
                return Err("UNKNOWN_CHOSEN_TOOL");
            }
            // LM Studio accepts only string choices. Restrict the tool set before requiring a call.
            body.get_mut("tools")
                .and_then(Value::as_array_mut)
                .ok_or("INVALID_TOOLS")?
                .retain(|t| t["name"] == name);
            body["tool_choice"] = json!("required");
        } else if !["auto", "none", "required"].contains(&choice.as_str().unwrap_or("")) {
            return Err("UNSUPPORTED_TOOL_CHOICE");
        }
    }
    let obj = body.as_object_mut().ok_or("INVALID_REQUEST")?;
    obj.insert("stream".into(), json!(false));
    obj.insert("store".into(), json!(false));
    obj.insert(
        "max_output_tokens".into(),
        json!(
            obj.get("max_output_tokens")
                .and_then(Value::as_u64)
                .unwrap_or(256)
        ),
    );
    // Stateless text/tool inference only. No OpenAI encrypted reasoning or hosted tools.
    for key in [
        "include",
        "reasoning",
        "prompt_cache_key",
        "safety_identifier",
        "service_tier",
        "metadata",
        "stream_options",
    ] {
        obj.remove(key);
    }
    Ok(Prepared {
        body,
        custom_tools,
        streaming,
        namespaces,
    })
}

pub fn restore_namespaces(response: &mut Value, namespaces: &BTreeMap<String, (String, String)>) {
    if let Some(items) = response.get_mut("output").and_then(Value::as_array_mut) {
        for item in items {
            if let Some((namespace, name)) = item
                .get("name")
                .and_then(Value::as_str)
                .and_then(|n| namespaces.get(n))
            {
                item["name"] = json!(name);
                item["namespace"] = json!(namespace);
            }
        }
    }
}

pub fn restore(mut response: Value, custom: &BTreeSet<String>) -> Result<Value, &'static str> {
    if response.get("object").and_then(Value::as_str) != Some("response")
        || !response.get("id").is_some_and(Value::is_string)
    {
        return Err("INVALID_UPSTREAM_RESPONSE");
    }
    if response["status"] != "completed" {
        return Err("UPSTREAM_NOT_COMPLETED");
    }
    if response.get("model").and_then(Value::as_str) != Some(MODEL) {
        return Err("UPSTREAM_MODEL_MISMATCH");
    }
    for item in response
        .get_mut("output")
        .and_then(Value::as_array_mut)
        .ok_or("INVALID_UPSTREAM_RESPONSE")?
    {
        if item["type"] == "function_call"
            && item
                .get("name")
                .and_then(Value::as_str)
                .is_some_and(|n| custom.contains(n))
        {
            let args: Value = serde_json::from_str(
                item["arguments"]
                    .as_str()
                    .ok_or("INVALID_CUSTOM_ARGUMENTS")?,
            )
            .map_err(|_| "INVALID_CUSTOM_ARGUMENTS")?;
            let input = args
                .get("input")
                .and_then(Value::as_str)
                .ok_or("INVALID_CUSTOM_ARGUMENTS")?;
            if args.as_object().is_none_or(|o| o.len() != 1) {
                return Err("INVALID_CUSTOM_ARGUMENTS");
            }
            item["input"] = json!(input);
            item["type"] = json!("custom_tool_call");
            item.as_object_mut()
                .ok_or("INVALID_CUSTOM_ARGUMENTS")?
                .remove("arguments");
        }
    }
    Ok(response)
}

// Buffered SSE: preserve Codex item events and usage; no invented token deltas.
pub fn sse(response: &Value) -> Result<String, &'static str> {
    let mut events = Vec::<Value>::new();
    let mut initial = response.clone();
    initial["status"] = json!("in_progress");
    initial["output"] = json!([]);
    initial["usage"] = Value::Null;
    events.push(json!({"type":"response.created","response":initial}));
    for (index, item) in response["output"]
        .as_array()
        .ok_or("INVALID_UPSTREAM_RESPONSE")?
        .iter()
        .enumerate()
    {
        let mut added = item.clone();
        added["status"] = json!("in_progress");
        if item["type"] == "function_call" {
            added["arguments"] = json!("");
        }
        if item["type"] == "custom_tool_call" {
            added["input"] = json!("");
        }
        if item["type"] == "message" {
            added["content"] = json!([]);
        }
        events.push(json!({"type":"response.output_item.added","output_index":index,"item":added}));
        match item["type"].as_str() {
            Some("message") => {
                for (part_index, part) in item["content"]
                    .as_array()
                    .ok_or("INVALID_UPSTREAM_RESPONSE")?
                    .iter()
                    .enumerate()
                {
                    if part["type"] != "output_text" {
                        return Err("UNSUPPORTED_UPSTREAM_CONTENT");
                    }
                    let text = part["text"].as_str().ok_or("INVALID_UPSTREAM_RESPONSE")?;
                    events.push(json!({"type":"response.content_part.added","item_id":item["id"],"output_index":index,"content_index":part_index,"part":{"type":"output_text","text":"","annotations":[]}}));
                    events.push(json!({"type":"response.output_text.delta","item_id":item["id"],"output_index":index,"content_index":part_index,"delta":text}));
                    events.push(json!({"type":"response.output_text.done","item_id":item["id"],"output_index":index,"content_index":part_index,"text":text}));
                    events.push(json!({"type":"response.content_part.done","item_id":item["id"],"output_index":index,"content_index":part_index,"part":part}));
                }
            }
            Some("function_call") => {
                events.push(json!({"type":"response.function_call_arguments.delta","item_id":item["id"],"output_index":index,"delta":item["arguments"]}));
                events.push(json!({"type":"response.function_call_arguments.done","item_id":item["id"],"output_index":index,"arguments":item["arguments"]}));
            }
            Some("custom_tool_call") => {
                events.push(json!({"type":"response.custom_tool_call_input.delta","item_id":item["id"],"output_index":index,"delta":item["input"]}));
                events.push(json!({"type":"response.custom_tool_call_input.done","item_id":item["id"],"output_index":index,"input":item["input"]}));
            }
            _ => return Err("UNSUPPORTED_UPSTREAM_ITEM"),
        }
        events.push(json!({"type":"response.output_item.done","output_index":index,"item":item}));
    }
    events.push(json!({"type":"response.completed","response":response}));
    Ok(events
        .into_iter()
        .enumerate()
        .map(|(i, mut event)| {
            event["sequence_number"] = json!(i);
            format!(
                "event: {}\ndata: {}\n\n",
                event["type"].as_str().unwrap_or("error"),
                event
            )
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> Value {
        json!({"model":MODEL,"input":"Hello","stream":true})
    }
    #[test]
    fn fixed_model_and_stateless_only() {
        let mut r = request();
        r["model"] = json!("other");
        assert_eq!(prepare(r).err(), Some("MODEL_NOT_ALLOWED"));
        let mut r = request();
        r["previous_response_id"] = json!("resp_old");
        assert_eq!(prepare(r).err(), Some("STATEFUL_REQUEST_UNSUPPORTED"));
    }
    #[test]
    fn named_tool_choice_restricts_upstream_tool_set() {
        let mut r = request();
        r["tools"] = json!([{"type":"custom","name":"chosen"},{"type":"custom","name":"other"}]);
        r["tool_choice"] = json!({"type":"custom","name":"chosen"});
        let p = prepare(r).unwrap();
        assert_eq!(p.body["tool_choice"], "required");
        assert_eq!(p.body["tools"].as_array().unwrap().len(), 1);
        assert_eq!(p.body["tools"][0]["name"], "chosen");
    }
    #[test]
    fn namespace_roundtrip_preserves_tool_identity() {
        let mut r = request();
        r["tools"] = json!([{"type":"namespace","name":"functions","tools":[{"type":"custom","name":"apply_patch"}]}]);
        r["input"] = json!([{"type":"custom_tool_call","namespace":"functions","name":"apply_patch","call_id":"c","input":"patch"},{"type":"custom_tool_call_output","call_id":"c","output":"ok"}]);
        let p = prepare(r).unwrap();
        assert_eq!(p.body["tools"][0]["name"], "functions__apply_patch");
        assert_eq!(p.body["input"][0]["name"], "functions__apply_patch");
        let mut response=restore(json!({"object":"response","id":"r","model":MODEL,"status":"completed","output":[{"id":"i","type":"function_call","name":"functions__apply_patch","call_id":"c","arguments":"{\"input\":\"patch\"}"}]}),&p.custom_tools).unwrap();
        restore_namespaces(&mut response, &p.namespaces);
        assert_eq!(response["output"][0]["namespace"], "functions");
        assert_eq!(response["output"][0]["name"], "apply_patch");
    }
    #[test]
    fn tool_translation_preserves_custom_input() {
        let mut r = request();
        r["tools"] = json!([{"type":"custom","name":"apply_patch"}]);
        r["input"] = json!([{"type":"custom_tool_call","name":"apply_patch","call_id":"c1","input":"*** Begin Patch"},{"type":"custom_tool_call_output","call_id":"c1","output":"done"}]);
        let p = prepare(r).unwrap();
        assert_eq!(p.body["tools"][0]["type"], "function");
        assert_eq!(p.body["input"][1]["type"], "function_call_output");
        let response = json!({"object":"response","id":"resp_test","model":MODEL,"status":"completed","output":[{"id":"item1","type":"function_call","name":"apply_patch","call_id":"c1","arguments":"{\"input\":\"*** Begin Patch\"}"}]});
        let restored = restore(response, &p.custom_tools).unwrap();
        assert_eq!(restored["output"][0]["input"], "*** Begin Patch");
        assert_eq!(restored["output"][0]["type"], "custom_tool_call");
        let stream = sse(&restored).unwrap();
        assert!(stream.contains("response.custom_tool_call_input.done"));
        assert!(stream.contains("response.completed"));
    }
    #[test]
    fn reject_hosted_tools_and_images() {
        let mut r = request();
        r["tools"] = json!([{"type":"web_search","name":"web"}]);
        assert!(prepare(r).is_err());
        let mut r = request();
        r["input"] = json!([{"role":"user","content":[{"type":"input_image","image_url":"https://example.com"}]}]);
        assert_eq!(prepare(r).err(), Some("UNSUPPORTED_CONTENT"));
    }
    #[test]
    fn bad_custom_arguments_fail_closed() {
        let response = json!({"object":"response","id":"r","model":MODEL,"status":"completed","output":[{"type":"function_call","name":"apply_patch","arguments":"{\"command\":\"wrong\"}"}]});
        assert_eq!(
            restore(response, &BTreeSet::from(["apply_patch".into()])).err(),
            Some("INVALID_CUSTOM_ARGUMENTS")
        );
    }
    #[test]
    fn bounded_output_and_incomplete_responses() {
        let mut r = request();
        r["max_output_tokens"] = json!(513);
        assert_eq!(prepare(r).err(), Some("OUTPUT_BUDGET_EXCEEDED"));
        let response =
            json!({"object":"response","id":"r","model":MODEL,"status":"incomplete","output":[]});
        assert_eq!(
            restore(response, &BTreeSet::new()).err(),
            Some("UPSTREAM_NOT_COMPLETED")
        );
    }
    #[test]
    fn text_stream_preserves_usage_and_output() {
        let response = json!({"object":"response","id":"r","model":MODEL,"status":"completed","usage":{"input_tokens":2,"output_tokens":1,"total_tokens":3},"output":[{"id":"m1","type":"message","role":"assistant","content":[{"type":"output_text","text":"OK","annotations":[]}]}]});
        let events = sse(&response).unwrap();
        assert!(events.contains("response.output_text.delta"));
        let last = events.split("data: ").last().unwrap().trim();
        let event: Value = serde_json::from_str(last).unwrap();
        assert_eq!(event["response"], response);
    }
}
