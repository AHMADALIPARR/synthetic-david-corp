//! Paid execution entitlement verification. No payment collection or bank execution.
use aes_gcm::{
    Aes256Gcm, KeyInit, Nonce,
    aead::{Aead, Payload},
};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
use zeroize::Zeroizing;

pub type Result<T> = std::result::Result<T, &'static str>;
pub const PRODUCT: &str = "SYNTHETIC-DAVID-CORP";
pub const MAX_LEASE_SECONDS: u64 = 86_400;
const DOMAIN: &[u8] = b"SNAPKITTY-EXECUTION-ENTITLEMENT-V1\0";
const TRUST_ROOT: &str = include_str!("../../../config/licensing-root.hex");
const FEATURES: &[&str] = &["corporate.validation", "qwen.responses", "codex.launch"];

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Payment {
    pub status: String,
    pub receipt_id: String,
    pub evidence_sha256: String,
    pub paid_from: u64,
    pub paid_through: u64,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Claims {
    pub version: u32,
    pub entitlement_id: String,
    pub licensee: String,
    pub product: String,
    pub deployment_sha256: String,
    pub features: Vec<String>,
    pub issued_at: u64,
    pub not_before: u64,
    pub expires_at: u64,
    pub payment: Payment,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Signed {
    claims: Claims,
    signature: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    version: u32,
    algorithm: String,
    nonce: String,
    ciphertext: String,
}

/// Constructed only after authenticated decryption, issuer signature and scope checks.
pub struct Permit {
    claims: Claims,
    issuer_sha256: String,
}
impl Permit {
    pub fn require(&self, feature: &str, now: u64) -> Result<()> {
        validate_claims(&self.claims, now)?;
        if !self.claims.features.iter().any(|f| f == feature) {
            return Err("LICENSE_FEATURE_DENIED");
        }
        Ok(())
    }
    pub fn summary(&self) -> serde_json::Value {
        serde_json::json!({"authorized":true,"entitlementId":self.claims.entitlement_id,"product":PRODUCT,"issuerSha256":self.issuer_sha256,"expiresAt":self.claims.expires_at,"paidThrough":self.claims.payment.paid_through,"features":self.claims.features})
    }
}
pub fn now() -> Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .map_err(|_| "LICENSE_CLOCK_INVALID")
}
fn text(s: &str, max: usize) -> bool {
    !s.trim().is_empty() && s.len() <= max && !s.chars().any(char::is_control)
}
fn digest(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn validate_claims(c: &Claims, time: u64) -> Result<()> {
    if c.version != 1
        || c.product != PRODUCT
        || !text(&c.entitlement_id, 64)
        || !text(&c.licensee, 128)
        || !digest(&c.deployment_sha256)
        || c.features.is_empty()
        || c.features.len() > FEATURES.len()
        || c.features.iter().any(|f| !FEATURES.contains(&f.as_str()))
        || c.features
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != c.features.len()
    {
        return Err("LICENSE_SCHEMA_INVALID");
    }
    if c.issued_at > c.not_before
        || c.not_before >= c.expires_at
        || c.expires_at.saturating_sub(c.issued_at) > MAX_LEASE_SECONDS
    {
        return Err("LICENSE_PERIOD_INVALID");
    }
    if time < c.not_before || time < c.issued_at {
        return Err("LICENSE_NOT_YET_VALID");
    }
    if time >= c.expires_at {
        return Err("LICENSE_EXPIRED");
    }
    let p = &c.payment;
    if p.status != "SETTLED"
        || !text(&p.receipt_id, 128)
        || !digest(&p.evidence_sha256)
        || p.paid_from >= p.paid_through
        || p.paid_from > c.not_before
        || p.paid_through < c.expires_at
        || time < p.paid_from
        || time >= p.paid_through
    {
        return Err("LICENSE_PAYMENT_REQUIRED");
    }
    Ok(())
}
fn signing_bytes(c: &Claims) -> Result<Vec<u8>> {
    let mut bytes = DOMAIN.to_vec();
    bytes.extend(serde_json::to_vec(c).map_err(|_| "LICENSE_SCHEMA_INVALID")?);
    Ok(bytes)
}
fn aad(deployment: &str) -> Vec<u8> {
    [
        DOMAIN,
        PRODUCT.as_bytes(),
        b"\0AES-256-GCM\0",
        deployment.as_bytes(),
    ]
    .concat()
}
pub fn verify(
    encrypted: &[u8],
    aes_key: &[u8; 32],
    issuer: &[u8; 32],
    deployment: &str,
    time: u64,
) -> Result<Permit> {
    if encrypted.len() > 32_768 || !digest(deployment) {
        return Err("LICENSE_SCHEMA_INVALID");
    }
    let envelope: Envelope =
        serde_json::from_slice(encrypted).map_err(|_| "LICENSE_SCHEMA_INVALID")?;
    if envelope.version != 1 || envelope.algorithm != "AES-256-GCM" {
        return Err("LICENSE_ALGORITHM_DENIED");
    }
    let nonce = hex::decode(envelope.nonce).map_err(|_| "LICENSE_SCHEMA_INVALID")?;
    if nonce.len() != 12 {
        return Err("LICENSE_SCHEMA_INVALID");
    }
    let ciphertext = hex::decode(envelope.ciphertext).map_err(|_| "LICENSE_SCHEMA_INVALID")?;
    let cipher = Aes256Gcm::new_from_slice(aes_key).map_err(|_| "LICENSE_KEY_INVALID")?;
    let plaintext = Zeroizing::new(
        cipher
            .decrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: &ciphertext,
                    aad: &aad(deployment),
                },
            )
            .map_err(|_| "LICENSE_AUTHENTICATION_FAILED")?,
    );
    let signed: Signed =
        serde_json::from_slice(&plaintext).map_err(|_| "LICENSE_SCHEMA_INVALID")?;
    let key = VerifyingKey::from_bytes(issuer).map_err(|_| "LICENSE_ISSUER_INVALID")?;
    if key.is_weak() {
        return Err("LICENSE_ISSUER_INVALID");
    }
    let raw = hex::decode(signed.signature).map_err(|_| "LICENSE_SIGNATURE_INVALID")?;
    let signature = Signature::from_slice(&raw).map_err(|_| "LICENSE_SIGNATURE_INVALID")?;
    key.verify_strict(&signing_bytes(&signed.claims)?, &signature)
        .map_err(|_| "LICENSE_SIGNATURE_INVALID")?;
    if signed.claims.deployment_sha256 != deployment {
        return Err("LICENSE_DEPLOYMENT_MISMATCH");
    }
    validate_claims(&signed.claims, time)?;
    Ok(Permit {
        claims: signed.claims,
        issuer_sha256: hex::encode(Sha256::digest(issuer)),
    })
}
/// Issuer-side only: caller must independently verify settlement before issuing.
/// Production deployments receive only the issuer PUBLIC key.
pub fn issue(
    claims: Claims,
    aes_key: &[u8; 32],
    signing_seed: &[u8; 32],
    time: u64,
) -> Result<Vec<u8>> {
    let signed = sign(claims.clone(), signing_seed, time)?;
    seal(
        &signed,
        aes_key,
        &SigningKey::from_bytes(signing_seed)
            .verifying_key()
            .to_bytes(),
        &claims.deployment_sha256,
        time,
    )
}
pub fn sign(claims: Claims, signing_seed: &[u8; 32], time: u64) -> Result<Vec<u8>> {
    validate_claims(&claims, time)?;
    let signature = SigningKey::from_bytes(signing_seed).sign(&signing_bytes(&claims)?);
    serde_json::to_vec(&Signed {
        claims,
        signature: hex::encode(signature.to_bytes()),
    })
    .map_err(|_| "LICENSE_SCHEMA_INVALID")
}
pub fn seal(
    signed: &[u8],
    aes_key: &[u8; 32],
    issuer: &[u8; 32],
    deployment: &str,
    time: u64,
) -> Result<Vec<u8>> {
    if signed.len() > 8192 || !digest(deployment) {
        return Err("LICENSE_SCHEMA_INVALID");
    }
    let mut nonce = [0u8; 12];
    getrandom::fill(&mut nonce).map_err(|_| "LICENSE_RANDOM_FAILED")?;
    let ciphertext = Aes256Gcm::new_from_slice(aes_key)
        .map_err(|_| "LICENSE_KEY_INVALID")?
        .encrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: signed,
                aad: &aad(deployment),
            },
        )
        .map_err(|_| "LICENSE_ENCRYPTION_FAILED")?;
    let blob = serde_json::to_vec(&Envelope {
        version: 1,
        algorithm: "AES-256-GCM".into(),
        nonce: hex::encode(nonce),
        ciphertext: hex::encode(ciphertext),
    })
    .map_err(|_| "LICENSE_SCHEMA_INVALID")?;
    verify(&blob, aes_key, issuer, deployment, time)?;
    Ok(blob)
}
fn read_bounded(path: &Path, max: usize) -> Result<Vec<u8>> {
    let file = fs::File::open(path).map_err(|_| "LICENSE_FILE_UNAVAILABLE")?;
    let mut bytes = Vec::new();
    file.take(max as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "LICENSE_FILE_UNAVAILABLE")?;
    if bytes.len() > max {
        return Err("LICENSE_FILE_TOO_LARGE");
    }
    Ok(bytes)
}
pub fn deployment_id() -> Result<String> {
    #[cfg(windows)]
    let identity: String = {
        use winreg::{
            RegKey,
            enums::{HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_64KEY},
        };
        RegKey::predef(HKEY_LOCAL_MACHINE)
            .open_subkey_with_flags(
                "SOFTWARE\\Microsoft\\Cryptography",
                KEY_READ | KEY_WOW64_64KEY,
            )
            .and_then(|k| k.get_value("MachineGuid"))
            .map_err(|_| "LICENSE_DEPLOYMENT_UNAVAILABLE")?
    };
    #[cfg(target_os = "linux")]
    let identity = String::from_utf8(read_bounded(Path::new("/etc/machine-id"), 128)?)
        .map_err(|_| "LICENSE_DEPLOYMENT_UNAVAILABLE")?;
    #[cfg(not(any(windows, target_os = "linux")))]
    let identity: String = return Err("LICENSE_DEPLOYMENT_UNSUPPORTED");
    if identity.trim().is_empty() {
        return Err("LICENSE_DEPLOYMENT_UNAVAILABLE");
    }
    Ok(hex::encode(Sha256::digest(
        [
            DOMAIN,
            std::env::consts::OS.as_bytes(),
            b"\0",
            identity.trim().as_bytes(),
        ]
        .concat(),
    )))
}
pub fn authorize(feature: &str) -> Result<Permit> {
    let issuer = issuer_root()?;
    let envelope =
        std::env::var_os("DAVID_ENTITLEMENT_FILE").ok_or("LICENSE_ENTITLEMENT_MISSING")?;
    let key = std::env::var_os("DAVID_DEPLOYMENT_KEY_FILE").ok_or("LICENSE_KEY_MISSING")?;
    let key = read_deployment_key(Path::new(&key))?;
    let time = now()?;
    let permit = verify(
        &read_bounded(Path::new(&envelope), 32_768)?,
        &key,
        &issuer,
        &deployment_id()?,
        time,
    )?;
    permit.require(feature, time)?;
    Ok(permit)
}
pub fn issuer_root() -> Result<[u8; 32]> {
    hex::decode(TRUST_ROOT.trim())
        .ok()
        .and_then(|v| v.try_into().ok())
        .ok_or("LICENSE_ISSUER_UNCONFIGURED")
}
pub fn status(feature: &str) -> serde_json::Value {
    match authorize(feature) {
        Ok(p) => p.summary(),
        Err(code) => serde_json::json!({"authorized":false,"errorCode":code}),
    }
}

#[cfg(windows)]
fn protect(input: &[u8], decrypt: bool) -> Result<Zeroizing<Vec<u8>>> {
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::Cryptography::{
            CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
        },
    };
    let mut input = Zeroizing::new(input.to_vec());
    let blob = CRYPT_INTEGER_BLOB {
        cbData: input.len().try_into().map_err(|_| "LICENSE_KEY_INVALID")?,
        pbData: input.as_mut_ptr(),
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    // DPAPI allocates output with LocalAlloc. Copy it, erase it, then LocalFree.
    unsafe {
        let ok = if decrypt {
            CryptUnprotectData(
                &blob,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptProtectData(
                &blob,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        };
        if ok == 0 {
            return Err("LICENSE_DEVICE_KEY_UNAVAILABLE");
        }
        let output_slice = std::slice::from_raw_parts_mut(output.pbData, output.cbData as usize);
        let bytes = Zeroizing::new(output_slice.to_vec());
        zeroize::Zeroize::zeroize(output_slice);
        LocalFree(output.pbData as _);
        Ok(bytes)
    }
}
pub fn read_deployment_key(path: &Path) -> Result<Zeroizing<[u8; 32]>> {
    let encoded = Zeroizing::new(read_bounded(path, 16_384)?);
    #[cfg(windows)]
    let bytes = protect(&encoded, true)?;
    #[cfg(not(windows))]
    let bytes = {
        use std::os::unix::fs::PermissionsExt;
        if fs::metadata(path)
            .map_err(|_| "LICENSE_KEY_INVALID")?
            .permissions()
            .mode()
            & 0o077
            != 0
        {
            return Err("LICENSE_KEY_PERMISSIONS_INVALID");
        }
        encoded
    };
    let key: [u8; 32] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| "LICENSE_KEY_INVALID")?;
    Ok(Zeroizing::new(key))
}
pub fn create_deployment_key(path: &Path) -> Result<()> {
    let mut key = Zeroizing::new([0u8; 32]);
    getrandom::fill(key.as_mut()).map_err(|_| "LICENSE_RANDOM_FAILED")?;
    #[cfg(windows)]
    let bytes = protect(key.as_ref(), false)?;
    #[cfg(not(windows))]
    let bytes = Zeroizing::new(key.to_vec());
    write_new_private(path, &bytes)
}
pub fn write_new_private(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|_| "LICENSE_FILE_CREATE_FAILED")?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| "LICENSE_FILE_CREATE_FAILED")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn malicious_envelope(c: Claims) -> Vec<u8> {
        // Test-only construction models an authenticated but invalid issuer grant.
        let signature = SigningKey::from_bytes(&[9; 32]).sign(&signing_bytes(&c).unwrap());
        let plain = serde_json::to_vec(&Signed {
            claims: c,
            signature: hex::encode(signature.to_bytes()),
        })
        .unwrap();
        let nonce = [5u8; 12];
        let ciphertext = Aes256Gcm::new_from_slice(&[7; 32])
            .unwrap()
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: &plain,
                    aad: &aad(&"a".repeat(64)),
                },
            )
            .unwrap();
        serde_json::to_vec(&Envelope {
            version: 1,
            algorithm: "AES-256-GCM".into(),
            nonce: hex::encode(nonce),
            ciphertext: hex::encode(ciphertext),
        })
        .unwrap()
    }
    fn claims() -> Claims {
        Claims {
            version: 1,
            entitlement_id: "DEVELOPMENT-ENTITLEMENT-FIXTURE".into(),
            licensee: "TEST-ONLY".into(),
            product: PRODUCT.into(),
            deployment_sha256: "a".repeat(64),
            features: vec!["corporate.validation".into()],
            issued_at: 1000,
            not_before: 1000,
            expires_at: 2000,
            payment: Payment {
                status: "SETTLED".into(),
                receipt_id: "TEST-ONLY-NOT-A-PAYMENT".into(),
                evidence_sha256: "b".repeat(64),
                paid_from: 1000,
                paid_through: 2000,
            },
        }
    }
    fn check(c: Claims, time: u64, feature: &str) -> Result<()> {
        let blob = issue(c, &[7; 32], &[9; 32], 1000)?;
        verify(
            &blob,
            &[7; 32],
            &SigningKey::from_bytes(&[9; 32]).verifying_key().to_bytes(),
            &"a".repeat(64),
            time,
        )?
        .require(feature, time)
    }
    #[test]
    fn paid_scope_and_expiry() {
        assert!(check(claims(), 1500, "corporate.validation").is_ok());
        assert_eq!(
            check(claims(), 1500, "qwen.responses").err(),
            Some("LICENSE_FEATURE_DENIED")
        );
        assert_eq!(
            check(claims(), 2000, "corporate.validation").err(),
            Some("LICENSE_EXPIRED")
        );
        assert_eq!(
            check(claims(), 999, "corporate.validation").err(),
            Some("LICENSE_NOT_YET_VALID")
        );
    }
    #[test]
    fn nonpayment_is_not_an_entitlement() {
        for status in ["UNPAID", "PENDING", "REFUNDED", "REVOKED"] {
            let mut c = claims();
            c.payment.status = status.into();
            assert_eq!(
                verify(
                    &malicious_envelope(c.clone()),
                    &[7; 32],
                    &SigningKey::from_bytes(&[9; 32]).verifying_key().to_bytes(),
                    &"a".repeat(64),
                    1500
                )
                .err(),
                Some("LICENSE_PAYMENT_REQUIRED")
            );
            assert_eq!(
                issue(c, &[7; 32], &[9; 32], 1000).err(),
                Some("LICENSE_PAYMENT_REQUIRED")
            );
        }
        let mut c = claims();
        c.payment.paid_through = 1999;
        assert_eq!(
            issue(c, &[7; 32], &[9; 32], 1000).err(),
            Some("LICENSE_PAYMENT_REQUIRED")
        );
    }
    #[test]
    fn possessing_aes_key_does_not_authorize_forged_payment() {
        let signed = sign(claims(), &[9; 32], 1000).unwrap();
        let mut value: Value = serde_json::from_slice(&signed).unwrap();
        value["claims"]["payment"]["paid_through"] = serde_json::json!(999999);
        let key = SigningKey::from_bytes(&[9; 32]).verifying_key().to_bytes();
        assert_eq!(
            seal(
                &serde_json::to_vec(&value).unwrap(),
                &[7; 32],
                &key,
                &"a".repeat(64),
                1500
            )
            .err(),
            Some("LICENSE_SIGNATURE_INVALID")
        );
        let mut c = claims();
        c.deployment_sha256 = "c".repeat(64);
        assert_eq!(
            verify(
                &malicious_envelope(c),
                &[7; 32],
                &key,
                &"a".repeat(64),
                1500
            )
            .err(),
            Some("LICENSE_DEPLOYMENT_MISMATCH")
        );
    }
    #[test]
    fn ciphertext_key_issuer_and_clone_rejected() {
        let c = claims();
        let mut blob = issue(c, &[7; 32], &[9; 32], 1000).unwrap();
        let issuer = SigningKey::from_bytes(&[9; 32]).verifying_key().to_bytes();
        assert_eq!(
            verify(&blob, &[8; 32], &issuer, &"a".repeat(64), 1500).err(),
            Some("LICENSE_AUTHENTICATION_FAILED")
        );
        assert_eq!(
            verify(&blob, &[7; 32], &issuer, &"c".repeat(64), 1500).err(),
            Some("LICENSE_AUTHENTICATION_FAILED")
        );
        assert_eq!(
            verify(
                &blob,
                &[7; 32],
                &SigningKey::from_bytes(&[8; 32]).verifying_key().to_bytes(),
                &"a".repeat(64),
                1500
            )
            .err(),
            Some("LICENSE_SIGNATURE_INVALID")
        );
        let mut envelope: Value = serde_json::from_slice(&blob).unwrap();
        let cipher = envelope["ciphertext"].as_str().unwrap();
        envelope["ciphertext"] = serde_json::json!(format!(
            "{}{}",
            if &cipher[..2] == "00" { "01" } else { "00" },
            &cipher[2..]
        ));
        blob = serde_json::to_vec(&envelope).unwrap();
        assert_eq!(
            verify(&blob, &[7; 32], &issuer, &"a".repeat(64), 1500).err(),
            Some("LICENSE_AUTHENTICATION_FAILED")
        );
    }
    #[test]
    fn nonce_is_fresh_and_schema_is_strict() {
        let a = issue(claims(), &[7; 32], &[9; 32], 1000).unwrap();
        let b = issue(claims(), &[7; 32], &[9; 32], 1000).unwrap();
        assert_ne!(a, b);
        let mut v: Value = serde_json::from_slice(&a).unwrap();
        v["paid"] = serde_json::json!(true);
        assert_eq!(
            verify(
                &serde_json::to_vec(&v).unwrap(),
                &[7; 32],
                &[0; 32],
                &"a".repeat(64),
                1500
            )
            .err(),
            Some("LICENSE_SCHEMA_INVALID")
        );
        let mut c = claims();
        c.expires_at = 1000 + MAX_LEASE_SECONDS + 1;
        assert_eq!(
            issue(c, &[7; 32], &[9; 32], 1000).err(),
            Some("LICENSE_PERIOD_INVALID")
        );
    }
    #[test]
    fn deployment_key_roundtrip_no_overwrite() {
        let mut random = [0u8; 16];
        getrandom::fill(&mut random).unwrap();
        let path = std::env::temp_dir().join(format!("david-key-{}.key", hex::encode(random)));
        create_deployment_key(&path).unwrap();
        let key = read_deployment_key(&path).unwrap();
        assert_eq!(key.len(), 32);
        assert!(create_deployment_key(&path).is_err());
        fs::remove_file(path).unwrap();
    }
    use serde_json::Value;
}
