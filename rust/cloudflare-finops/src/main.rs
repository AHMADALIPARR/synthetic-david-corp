use david_cloudflare_finops::{
    Audit, advise, collect, config, dsml, gguf_hash, propose_dsml, query, read_json, windows,
};
use serde_json::{Value, json};
use std::io::Read;
use std::path::{Path, PathBuf};
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_owned()
}
fn run() -> Result<(), &'static str> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let action = args.first().map(String::as_str).unwrap_or("help");
    if action == "help" {
        println!(
            "david-cloudflare-finops plan [CONFIG] | dsml-example [CONFIG] | invoke FILE.dsml [CONFIG] [DB] | agent [CONFIG] [DB] | collect [CONFIG] [DB] | audit [DB] | advise REPORT_ID [CONFIG] [DB] | gguf-hash FILE"
        );
        return Ok(());
    }
    let result: Value = match action {
        "gguf-hash" => {
            json!({"ggufSha256":gguf_hash(Path::new(args.get(1).ok_or("GGUF_PATH_REQUIRED")?))?,"trainedWeightsVerified":false})
        }
        "dsml-example" => {
            let cfg = read_json(
                &args
                    .get(1)
                    .map(PathBuf::from)
                    .unwrap_or(root().join("config/cloudflare-finops.json")),
            )?;
            config(&cfg)?;
            println!("{}", dsml::example(cfg["zoneId"].as_str().unwrap()));
            return Ok(());
        }
        "invoke" | "agent" => {
            let offset = if action == "invoke" { 2 } else { 1 };
            let cfg = read_json(
                &args
                    .get(offset)
                    .map(PathBuf::from)
                    .unwrap_or(root().join("config/cloudflare-finops.json")),
            )?;
            config(&cfg)?;
            let proposal = if action == "invoke" {
                let mut bytes = Vec::new();
                std::fs::File::open(args.get(1).ok_or("DSML_FILE_REQUIRED")?)
                    .map_err(|_| "DSML_FILE_UNAVAILABLE")?
                    .take(2049)
                    .read_to_end(&mut bytes)
                    .map_err(|_| "DSML_FILE_UNAVAILABLE")?;
                let input = std::str::from_utf8(&bytes).map_err(|_| "DSML_ENCODING_INVALID")?;
                json!({"invocation":dsml::parse(input,cfg["zoneId"].as_str().unwrap())?,"proposalHash":david_control_plane::hash(&json!(input)),"executionAuthority":"RUST-TYPED-ALLOWLIST"})
            } else {
                propose_dsml(&cfg)?
            };
            let mut report = collect(&cfg, chrono::Utc::now().date_naive())?;
            report["plannerProposal"] = proposal;
            david_execution_gate::authorize("corporate.validation")?;
            let mut audit = Audit::open(
                &args
                    .get(offset + 1)
                    .map(PathBuf::from)
                    .unwrap_or(root().join("data/cloudflare-finops.sqlite")),
            )?;
            let receipt = audit.commit(&report)?;
            if action == "agent" {
                match advise(&cfg, &report) {
                    Ok(advice) => {
                        david_execution_gate::authorize("corporate.validation")?;
                        let advice_receipt = audit.commit_advice(&report, &advice)?;
                        json!({"receipt":receipt,"report":report,"adviceReceipt":advice_receipt,"advice":advice})
                    }
                    Err(code) => {
                        json!({"receipt":receipt,"report":report,"advisoryStatus":"UNAVAILABLE","advisoryError":code,"changesExecuted":false})
                    }
                }
            } else {
                json!({"receipt":receipt,"report":report})
            }
        }
        "plan" | "collect" => {
            let cfg = read_json(
                &args
                    .get(1)
                    .map(PathBuf::from)
                    .unwrap_or(root().join("config/cloudflare-finops.json")),
            )?;
            config(&cfg)?;
            if action == "plan" {
                let ranges = windows(chrono::Utc::now().date_naive());
                json!({"agentId":"CLOUDFLARE-FINOPS","domain":cfg["domain"],"queries":[query(cfg["zoneId"].as_str().unwrap(),&ranges[0])?,query(cfg["zoneId"].as_str().unwrap(),&ranges[1])?],"networkExecuted":false,"billingEnabled":cfg["billingEnabled"],"mimoConfigured":!cfg["mimo"].is_null()})
            } else {
                let report = collect(&cfg, chrono::Utc::now().date_naive())?;
                david_execution_gate::authorize("corporate.validation")?;
                let mut audit = Audit::open(
                    &args
                        .get(2)
                        .map(PathBuf::from)
                        .unwrap_or(root().join("data/cloudflare-finops.sqlite")),
                )?;
                let receipt = audit.commit(&report)?;
                json!({"receipt":receipt,"report":report})
            }
        }
        "audit" => Audit::open(
            &args
                .get(1)
                .map(PathBuf::from)
                .unwrap_or(root().join("data/cloudflare-finops.sqlite")),
        )?
        .verify()?,
        "advise" => {
            let id = args.get(1).ok_or("REPORT_ID_REQUIRED")?;
            let cfg = read_json(
                &args
                    .get(2)
                    .map(PathBuf::from)
                    .unwrap_or(root().join("config/cloudflare-finops.json")),
            )?;
            let mut audit = Audit::open(
                &args
                    .get(3)
                    .map(PathBuf::from)
                    .unwrap_or(root().join("data/cloudflare-finops.sqlite")),
            )?;
            let report = audit.report(id)?;
            let advice = advise(&cfg, &report)?;
            david_execution_gate::authorize("corporate.validation")?;
            let receipt = audit.commit_advice(&report, &advice)?;
            json!({"receipt":receipt,"advice":advice})
        }
        _ => return Err("UNKNOWN_ACTION"),
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&result).map_err(|_| "JSON_INVALID")?
    );
    Ok(())
}
fn main() {
    if let Err(code) = run() {
        eprintln!("{code}");
        std::process::exit(1);
    }
}
