//! Offline licensor/customer provisioning. Do not distribute the issuer private key.
use david_execution_gate as gate;
use ed25519_dalek::SigningKey;
use std::{io::Read, path::Path};

fn read(path: &str) -> Result<Vec<u8>, &'static str> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|_| "LICENSE_FILE_UNAVAILABLE")?
        .take(8193)
        .read_to_end(&mut bytes)
        .map_err(|_| "LICENSE_FILE_UNAVAILABLE")?;
    if bytes.len() > 8192 {
        return Err("LICENSE_FILE_TOO_LARGE");
    }
    Ok(bytes)
}
fn run() -> Result<(), &'static str> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["issuer-key", private, public] => {
            // The private key stays on the licensor's offline machine.
            gate::create_deployment_key(Path::new(private))?;
            let seed = gate::read_deployment_key(Path::new(private))?;
            gate::write_new_private(
                Path::new(public),
                hex::encode(SigningKey::from_bytes(&seed).verifying_key().to_bytes()).as_bytes(),
            )?;
            println!(
                "Issuer key created. Keep the private key offline; pin only the public key in config/licensing-root.hex and rebuild."
            );
        }
        ["deployment-key", path] => {
            gate::create_deployment_key(Path::new(path))?;
            println!("Deployment key created.");
        }
        ["deployment-id"] => println!("{}", gate::deployment_id()?),
        ["sign", private, claims, out] => {
            let claims: gate::Claims =
                serde_json::from_slice(&read(claims)?).map_err(|_| "LICENSE_SCHEMA_INVALID")?;
            let seed = gate::read_deployment_key(Path::new(private))?;
            let signed = gate::sign(claims, &seed, gate::now()?)?;
            gate::write_new_private(Path::new(out), &signed)?;
            println!(
                "Entitlement signed. Settlement evidence must have been independently verified by the licensor."
            );
        }
        ["install", signed, key, out] => {
            let key = gate::read_deployment_key(Path::new(key))?;
            let sealed = gate::seal(
                &read(signed)?,
                &key,
                &gate::issuer_root()?,
                &gate::deployment_id()?,
                gate::now()?,
            )?;
            gate::write_new_private(Path::new(out), &sealed)?;
            println!(
                "Issuer signature verified; entitlement sealed with AES-256-GCM for this deployment."
            );
        }
        ["help"] | [] => println!(
            "david-license-admin issuer-key PRIVATE PUBLIC | deployment-key FILE | deployment-id | sign PRIVATE CLAIMS OUTPUT | install SIGNED DEPLOYMENT_KEY OUTPUT"
        ),
        _ => return Err("LICENSE_ADMIN_USAGE_INVALID"),
    }
    Ok(())
}
fn main() {
    if let Err(code) = run() {
        eprintln!("{code}");
        std::process::exit(1);
    }
}
