use anyhow::{anyhow, Context};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::process::Command;

const PACKAGE_NAME: &str = "sanctifier-cli";

/// Trusted ed25519 public key (hex) that signs Sanctifier release manifests.
///
/// The matching secret key is held only by the release signers and is never
/// committed. The self-updater will refuse to install any release whose
/// manifest is not signed by this key, so a compromised registry or a
/// man-in-the-middle cannot push a malicious binary. Rotating the key is a
/// code change (this constant) shipped in a signed release.
const RELEASE_PUBLIC_KEY_HEX: &str =
    "2a5a0025a227325213b3e495be02f1615e0b2c557c78425544ef980e0e226ba0";

/// Base URL that hosts the signed release manifests. Overridable via
/// `SANCTIFIER_RELEASE_BASE_URL` for mirrors and integration tests.
const DEFAULT_RELEASE_BASE_URL: &str =
    "https://github.com/Centurylong/sanctifier/releases/download";

/// A signed release manifest: the version being published and the SHA-256
/// (hex) of the exact `.crate` artifact for that version.
#[derive(Debug, Clone, Deserialize)]
struct ReleaseManifest {
    version: String,
    sha256: String,
}

pub fn exec() -> anyhow::Result<()> {
    let current = env!("CARGO_PKG_VERSION");
    println!("Checking for Sanctifier updates (current: v{current})...");

    let latest = fetch_latest_version()?;
    if !is_newer_version(current, &latest) {
        println!("Sanctifier is already up to date (v{current}).");
        return Ok(());
    }

    println!("Updating Sanctifier from v{current} to v{latest}...");

    // Integrity gate (#810): never install a release we cannot cryptographically
    // verify. Any failure here — a missing manifest, a bad signature, a version
    // or hash mismatch — aborts the update *before* anything is installed.
    verify_release(&latest)
        .with_context(|| format!("refusing to install unverified release v{latest}"))?;
    println!("Signature verified. Installing v{latest}...");

    install_version(&latest)?;
    println!("Update complete. Sanctifier is now at version v{latest}.");
    Ok(())
}

fn release_base_url() -> String {
    std::env::var("SANCTIFIER_RELEASE_BASE_URL")
        .unwrap_or_else(|_| DEFAULT_RELEASE_BASE_URL.to_string())
}

/// Download the signed manifest + detached signature for `version`, verify the
/// signature against the embedded trusted key, then download the `.crate`
/// artifact and confirm its SHA-256 matches the manifest. Fails closed.
fn verify_release(version: &str) -> anyhow::Result<()> {
    let base = release_base_url();
    let manifest_url = format!("{base}/v{version}/manifest.json");
    let signature_url = format!("{base}/v{version}/manifest.json.sig");

    let manifest_bytes = http_get_bytes(&manifest_url)
        .with_context(|| format!("failed to download release manifest from {manifest_url}"))?;
    let signature_hex = http_get_text(&signature_url)
        .with_context(|| format!("failed to download release signature from {signature_url}"))?;

    verify_manifest_signature(RELEASE_PUBLIC_KEY_HEX, &manifest_bytes, signature_hex.trim())?;

    let manifest: ReleaseManifest =
        serde_json::from_slice(&manifest_bytes).context("release manifest is not valid JSON")?;

    if manifest.version != version {
        return Err(anyhow!(
            "signed manifest is for v{} but v{} was requested",
            manifest.version,
            version
        ));
    }

    let artifact_url = format!(
        "https://static.crates.io/crates/{PACKAGE_NAME}/{PACKAGE_NAME}-{version}.crate"
    );
    let artifact = http_get_bytes(&artifact_url)
        .with_context(|| format!("failed to download crate artifact from {artifact_url}"))?;
    let actual = sha256_hex(&artifact);
    if !constant_time_eq(actual.as_bytes(), manifest.sha256.trim().as_bytes()) {
        return Err(anyhow!(
            "artifact hash mismatch: manifest expects {} but downloaded artifact is {actual}",
            manifest.sha256.trim()
        ));
    }

    Ok(())
}

/// Verify a detached ed25519 signature (hex) over `message` against a trusted
/// public key (hex). Returns `Ok(())` only when the signature is valid.
fn verify_manifest_signature(
    pubkey_hex: &str,
    message: &[u8],
    signature_hex: &str,
) -> anyhow::Result<()> {
    let pubkey_bytes: [u8; 32] = hex::decode(pubkey_hex)
        .context("trusted public key is not valid hex")?
        .try_into()
        .map_err(|_| anyhow!("trusted public key must be 32 bytes"))?;
    let verifying_key =
        VerifyingKey::from_bytes(&pubkey_bytes).context("trusted public key is not a valid ed25519 key")?;

    let sig_bytes: [u8; 64] = hex::decode(signature_hex)
        .context("signature is not valid hex")?
        .try_into()
        .map_err(|_| anyhow!("signature must be 64 bytes"))?;
    let signature = Signature::from_bytes(&sig_bytes);

    verifying_key
        .verify_strict(message, &signature)
        .map_err(|_| anyhow!("signature does not match the trusted release key"))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

/// Length-independent, short-circuit-free byte comparison to avoid leaking the
/// position of a first difference when comparing hashes.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn http_get_bytes(url: &str) -> anyhow::Result<Vec<u8>> {
    let resp = reqwest::blocking::get(url)
        .with_context(|| format!("HTTP request to {url} failed"))?
        .error_for_status()
        .with_context(|| format!("HTTP request to {url} returned an error status"))?;
    Ok(resp.bytes()?.to_vec())
}

fn http_get_text(url: &str) -> anyhow::Result<String> {
    Ok(String::from_utf8(http_get_bytes(url)?)?)
}

fn fetch_latest_version() -> anyhow::Result<String> {
    let output = Command::new("cargo")
        .args(["search", PACKAGE_NAME, "--limit", "1"])
        .output()
        .context("failed to run `cargo search`")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("`cargo search` failed: {}", stderr.trim()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_latest_version(&stdout)
}

fn parse_latest_version(output: &str) -> anyhow::Result<String> {
    for line in output.lines() {
        if line.trim_start().starts_with(PACKAGE_NAME) {
            let mut parts = line.split('"');
            let _before = parts.next();
            if let Some(version) = parts.next() {
                let cleaned = version.trim().to_string();
                if !cleaned.is_empty() {
                    return Ok(cleaned);
                }
            }
        }
    }

    Err(anyhow!(
        "could not parse latest sanctifier-cli version from cargo search output"
    ))
}

fn install_version(version: &str) -> anyhow::Result<()> {
    let status = Command::new("cargo")
        .args([
            "install",
            PACKAGE_NAME,
            "--locked",
            "--force",
            "--version",
            version,
        ])
        .status()
        .context("failed to run `cargo install`")?;

    if status.success() {
        Ok(())
    } else {
        Err(anyhow!(
            "`cargo install` failed while installing sanctifier-cli v{}",
            version
        ))
    }
}

fn parse_triplet(version: &str) -> Option<(u64, u64, u64)> {
    let mut fields = version.split('.');
    let major = fields.next()?.parse::<u64>().ok()?;
    let minor = fields.next()?.parse::<u64>().ok()?;
    let patch_field = fields.next()?;
    let patch = patch_field
        .split(|c: char| !c.is_ascii_digit())
        .next()?
        .parse::<u64>()
        .ok()?;
    Some((major, minor, patch))
}

fn is_newer_version(current: &str, latest: &str) -> bool {
    match (parse_triplet(current), parse_triplet(latest)) {
        (Some(cur), Some(new)) => new > cur,
        _ => current.trim() != latest.trim(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    #[test]
    fn parse_triplet_parses_semver_values() {
        assert_eq!(parse_triplet("1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_triplet("1.2.3-beta.1"), Some((1, 2, 3)));
        assert_eq!(parse_triplet("1.2"), None);
    }

    #[test]
    fn parse_latest_version_extracts_first_match() {
        let output = "sanctifier-cli = \"0.3.4\"    # Sanctifier CLI";
        let version = parse_latest_version(output).unwrap();
        assert_eq!(version, "0.3.4");
    }

    #[test]
    fn parse_latest_version_errors_on_missing_match() {
        let output = "something-else = \"1.0.0\"";
        assert!(parse_latest_version(output).is_err());
    }

    #[test]
    fn version_compare_prefers_higher_triplet() {
        assert!(is_newer_version("0.1.0", "0.2.0"));
        assert!(!is_newer_version("0.3.0", "0.2.9"));
        assert!(!is_newer_version("0.1.0", "0.1.0"));
    }

    #[test]
    fn sha256_hex_matches_known_vector() {
        // SHA-256 of the empty input.
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn constant_time_eq_behaves_like_equality() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"abcd"));
    }

    fn test_keypair() -> SigningKey {
        // Deterministic key so the test is reproducible; production uses a
        // real, offline-held signing key.
        SigningKey::from_bytes(&[7u8; 32])
    }

    #[test]
    fn accepts_a_correctly_signed_manifest() {
        let key = test_keypair();
        let pubkey_hex = hex::encode(key.verifying_key().to_bytes());
        let manifest = br#"{"version":"1.2.3","sha256":"deadbeef"}"#;
        let sig_hex = hex::encode(key.sign(manifest).to_bytes());

        assert!(verify_manifest_signature(&pubkey_hex, manifest, &sig_hex).is_ok());
    }

    #[test]
    fn rejects_a_tampered_manifest() {
        let key = test_keypair();
        let pubkey_hex = hex::encode(key.verifying_key().to_bytes());
        let manifest = br#"{"version":"1.2.3","sha256":"deadbeef"}"#;
        let sig_hex = hex::encode(key.sign(manifest).to_bytes());

        // Same signature, different (attacker-substituted) manifest body.
        let tampered = br#"{"version":"6.6.6","sha256":"deadbeef"}"#;
        assert!(verify_manifest_signature(&pubkey_hex, tampered, &sig_hex).is_err());
    }

    #[test]
    fn rejects_a_signature_from_the_wrong_key() {
        let signer = test_keypair();
        let attacker = SigningKey::from_bytes(&[9u8; 32]);
        let trusted_pubkey_hex = hex::encode(signer.verifying_key().to_bytes());
        let manifest = br#"{"version":"1.2.3","sha256":"deadbeef"}"#;
        // Signed by the attacker, verified against the trusted key -> reject.
        let sig_hex = hex::encode(attacker.sign(manifest).to_bytes());

        assert!(verify_manifest_signature(&trusted_pubkey_hex, manifest, &sig_hex).is_err());
    }

    #[test]
    fn rejects_malformed_signature_and_key() {
        let key = test_keypair();
        let pubkey_hex = hex::encode(key.verifying_key().to_bytes());
        let manifest = b"anything";

        // Not hex.
        assert!(verify_manifest_signature(&pubkey_hex, manifest, "nothex").is_err());
        // Wrong length.
        assert!(verify_manifest_signature(&pubkey_hex, manifest, "abcd").is_err());
        // Bad trusted key.
        assert!(verify_manifest_signature("00", manifest, &"aa".repeat(64)).is_err());
    }

    #[test]
    fn embedded_release_key_is_a_valid_ed25519_key() {
        let bytes: [u8; 32] = hex::decode(RELEASE_PUBLIC_KEY_HEX)
            .expect("hex")
            .try_into()
            .expect("32 bytes");
        assert!(VerifyingKey::from_bytes(&bytes).is_ok());
    }
}
