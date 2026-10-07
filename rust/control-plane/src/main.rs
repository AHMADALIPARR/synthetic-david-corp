use david_control_plane::{Store, demo_policy, demo_request, execute};
use serde_json::{Value, json};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_owned()
}
fn read(path: impl AsRef<Path>) -> Result<Value, Box<dyn std::error::Error>> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}
fn health() -> Result<Value, Box<dyn std::error::Error>> {
    let v: Value = reqwest::blocking::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(2))
        .build()?
        .get("http://127.0.0.1:1235/health")
        .send()?
        .error_for_status()?
        .json()?;
    if v["service"] != "david-qwen-endpoint" {
        return Err("Unexpected adapter".into());
    }
    Ok(v)
}
fn start() -> Result<Value, Box<dyn std::error::Error>> {
    if let Ok(v) = health() {
        return Ok(v);
    }
    let root = root();
    fs::create_dir_all(root.join("data"))?;
    let mut command = Command::new(root.join(format!(
        "rust/target/debug/david-qwen-endpoint{}",
        std::env::consts::EXE_SUFFIX
    )));
    command
        .current_dir(&root)
        .stdin(Stdio::null())
        .stdout(
            fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(root.join("data/qwen-adapter.log"))?,
        )
        .stderr(
            fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(root.join("data/qwen-adapter-error.log"))?,
        );
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000 | 0x00000008);
    }
    let mut child = command.spawn()?;
    for _ in 0..20 {
        if let Ok(v) = health() {
            return Ok(v);
        }
        if child.try_wait()?.is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    Err("Adapter failed to start; inspect data/qwen-adapter-error.log".into())
}
fn codex(prompt: &[String]) -> Result<i32, Box<dyn std::error::Error>> {
    health()?;
    let root = root();
    let state = root.join(".codex-local");
    fs::create_dir_all(&state)?;
    let path_string =
        |p: PathBuf| serde_json::to_string(&p.to_string_lossy().replace('\\', "/")).unwrap();
    fs::write(
        state.join("config.toml"),
        format!(
            "model_instructions_file = {}\nmodel_catalog_json = {}\n{}",
            path_string(root.join("config/qwen-instructions.md")),
            path_string(root.join("config/qwen-models.json")),
            fs::read_to_string(root.join("config/codex-qwen.toml"))?
        ),
    )?;
    let executable = if let Some(p) = std::env::var_os("CODEX_CLI_BIN") {
        PathBuf::from(p)
    } else {
        #[cfg(windows)]
        {
            let packages =
                PathBuf::from(std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA missing")?)
                    .join("JetBrains/DataGrip2026.2/acp-agents/codex-acp");
            let mut versions: Vec<_> = fs::read_dir(packages)?
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .collect();
            versions.sort();
            versions.reverse();
            versions.into_iter().map(|v|v.join("node_modules/@openai/codex-win32-x64/vendor/x86_64-pc-windows-msvc/bin/codex.exe")).find(|p|p.is_file()).ok_or("Set CODEX_CLI_BIN to the native Codex executable")?
        }
        #[cfg(not(windows))]
        {
            PathBuf::from("codex")
        }
    };
    let executable = native_executable(executable)?;
    let mut child = Command::new(executable);
    child.current_dir(&root).env("CODEX_HOME", state);
    if !prompt.is_empty() {
        child
            .args(["exec", "--ephemeral", "--json"])
            .stdin(Stdio::null());
    }
    child.arg("-C").arg(&root).args(["--sandbox", "read-only"]);
    if !prompt.is_empty() {
        child.arg(prompt.join(" "));
    }
    Ok(child.status()?.code().unwrap_or(1))
}
fn native_executable(path: PathBuf) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let path = if path.components().count() == 1 && !path.is_file() {
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
            .map(|p| p.join(&path))
            .find(|p| p.is_file())
            .ok_or("Set CODEX_CLI_BIN to the native Codex executable")?
    } else {
        path
    };
    let mut magic = [0u8; 4];
    fs::File::open(&path)?.read_exact(&mut magic)?;
    if !native_magic(magic) {
        return Err("Codex must be a native executable; script wrappers are forbidden".into());
    }
    Ok(path)
}
fn native_magic(magic: [u8; 4]) -> bool {
    magic[..2] == *b"MZ"
        || magic == *b"\x7fELF"
        || matches!(
            magic,
            [0xfe, 0xed, 0xfa, 0xce]
                | [0xce, 0xfa, 0xed, 0xfe]
                | [0xfe, 0xed, 0xfa, 0xcf]
                | [0xcf, 0xfa, 0xed, 0xfe]
                | [0xca, 0xfe, 0xba, 0xbe]
                | [0xbe, 0xba, 0xfe, 0xca]
        )
}
#[cfg(test)]
mod tests {
    #[test]
    fn native_launcher_rejects_script_wrappers() {
        assert!(!super::native_magic(*b"#!/u"));
        assert!(!super::native_magic(*b"@ech"));
        assert!(super::native_magic(*b"\x7fELF"));
        assert!(super::native_magic([b'M', b'Z', 0, 0]));
    }
}
fn verify_qwen() -> Result<Value, Box<dyn std::error::Error>> {
    let client = reqwest::blocking::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(110))
        .build()?;
    let send = |body: Value| {
        client
            .post("http://127.0.0.1:1235/v1/responses")
            .json(&body)
            .send()
    };
    let text = send(json!({"model":"qwen-local","input":"Reply with exactly OK. /no_think","max_output_tokens":32,"stream":true}))?.error_for_status()?;
    if !text
        .headers()
        .get("content-type")
        .is_some_and(|h| h.to_str().unwrap_or("").contains("text/event-stream"))
    {
        return Err("Expected Responses SSE".into());
    }
    let events: Vec<Value> = text
        .text()?
        .lines()
        .filter_map(|s| s.strip_prefix("data: "))
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    if !events
        .iter()
        .any(|e| e["type"] == "response.output_text.delta")
        || !events.iter().any(|e| {
            e["type"] == "response.completed"
                && e["response"]["usage"]["total_tokens"].as_u64().unwrap_or(0) > 0
        })
    {
        return Err("Missing real text or usage".into());
    }
    let tool: Value = send(json!({"model":"qwen-local","input":"Call evidence_check with input equal to test-fixture. /no_think","max_output_tokens":160,"tools":[{"type":"custom","name":"evidence_check","description":"Development wire fixture; accepts a short text string."}],"tool_choice":{"type":"custom","name":"evidence_check"}}))?.error_for_status()?.json()?;
    let call = tool["output"]
        .as_array()
        .and_then(|a| {
            a.iter().find(|v| {
                v["type"] == "custom_tool_call"
                    && v["name"] == "evidence_check"
                    && v["input"].is_string()
            })
        })
        .ok_or("Missing custom tool proposal")?;
    let continued: Value = send(json!({"model":"qwen-local","input":[{"role":"user","content":"Check the development wire fixture."},call,{"type":"custom_tool_call_output","call_id":call["call_id"],"output":"DEVELOPMENT-WIRE-FIXTURE-OK"},{"role":"user","content":"Reply with exactly OK. /no_think"}],"max_output_tokens":32}))?.error_for_status()?.json()?;
    if !continued["output"]
        .as_array()
        .is_some_and(|a| a.iter().any(|v| v["type"] == "message"))
    {
        return Err("Missing continuation message".into());
    }
    if send(json!({"model":"unapproved-model","input":"hello"}))?
        .status()
        .as_u16()
        != 400
    {
        return Err("Unknown model accepted".into());
    }
    Ok(
        json!({"textSse":true,"actualUsage":true,"customToolProposal":true,"resultContinuation":true,"unknownModelRejected":true,"toolExecuted":false}),
    )
}
fn publication() -> Result<Value, Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .args(["ls-files", "--stage", "-z"])
        .current_dir(root())
        .output()?;
    if !output.status.success() {
        return Err("git ls-files failed".into());
    }
    let sensitive = regex::Regex::new(
        r"(?i)(^|/)(\.env(\..*)?|\.tools|\.codex-local|node_modules|target|data|build)(/|$)|\.(log|db|sqlite.*|exe|dll|gguf|bin|mjs|js|ts|ps1)$",
    )?;
    let secrets = regex::Regex::new(
        r"\b(gh[pousr]_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{40,}|sk-(proj-|ant-)?[A-Za-z0-9_-]{40,})|-----BEGIN (RSA |EC |OPENSSH )?PRIVATE KEY-----",
    )?;
    let (mut count, mut bytes) = (0, 0);
    for entry in std::str::from_utf8(&output.stdout)?
        .split('\0')
        .filter(|e| !e.is_empty())
    {
        let (meta, file) = entry.split_once('\t').ok_or("Invalid git index")?;
        if meta.starts_with("160000 ") {
            if file != "vendor/codex" {
                return Err("Unexpected submodule".into());
            }
            continue;
        }
        if sensitive.is_match(file) {
            return Err(
                format!("Local artifact or forbidden implementation staged: {file}").into(),
            );
        }
        let content = Command::new("git")
            .args(["show", &format!(":{file}")])
            .current_dir(root())
            .output()?;
        if !content.status.success()
            || content.stdout.len() > 10 * 1024 * 1024
            || content.stdout.contains(&0)
            || secrets.is_match(std::str::from_utf8(&content.stdout)?)
        {
            return Err(format!("Publication scan failed: {file}").into());
        }
        count += 1;
        bytes += content.stdout.len();
    }
    Ok(
        json!({"checkedFiles":count,"totalBytes":bytes,"pinnedSubmodule":"vendor/codex","localArtifactsExcluded":true}),
    )
}
fn main() {
    let result = run();
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let value = match args.first().map(String::as_str).unwrap_or("help") {
        "demo" => execute(&Store::open(":memory:")?, &demo_policy(), &demo_request()),
        "run" => {
            let request = read(
                args.get(1)
                    .ok_or("Usage: david run REQUEST.json [POLICY.json] [DATABASE]")?,
            )?;
            let policy = read(
                args.get(2)
                    .map(PathBuf::from)
                    .unwrap_or(root().join("config/principals.json")),
            )?;
            let store = Store::open(
                &args.get(3).cloned().unwrap_or(
                    root()
                        .join("data/control-plane.sqlite")
                        .to_string_lossy()
                        .into_owned(),
                ),
            )?;
            let response = execute(&store, &policy, &request);
            println!("{}", serde_json::to_string_pretty(&response)?);
            if response["status"] != "COMPLETED" {
                std::process::exit(1);
            }
            return Ok(());
        }
        "audit" => Store::open(
            &args.get(1).cloned().unwrap_or(
                root()
                    .join("data/control-plane.sqlite")
                    .to_string_lossy()
                    .into_owned(),
            ),
        )?
        .verify()?,
        "qwen-start" => start()?,
        "qwen-status" => health()?,
        "qwen-verify" => verify_qwen()?,
        "publication-check" => publication()?,
        "codex" => std::process::exit(codex(&args[1..])?),
        "help" => {
            println!(
                "david demo | run REQUEST [POLICY] [DB] | audit [DB] | qwen-start | qwen-status | qwen-verify | codex [PROMPT] | publication-check"
            );
            return Ok(());
        }
        _ => return Err("Unknown command".into()),
    };
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}
