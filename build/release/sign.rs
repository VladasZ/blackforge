// Signs one release artifact for the engine updater. Writes `<file>.meta.json`
// with the size, sha256 and ed25519 signature, the fields updater.json embeds
// per platform. The secret is `<NAME>_UPDATE_KEY` in the env, 32 bytes hex, or
// the local key file when unset.

use anyhow::{Context, Result, bail, ensure};
use ed25519_dalek::{Signer, SigningKey};
use serde::Serialize;
use sha2::{Digest, Sha256};
use shared::release;
use std::env::args;
use std::env::var;
use std::fs::read;
use std::fs::read_to_string;
use std::fs::write;

#[derive(Serialize)]
struct Meta {
    size: u64,
    sha256: String,
    sig: String,
}

fn main() -> Result<()> {
    let release = release::read()?;
    let files: Vec<String> = args().skip(1).collect();
    if files.is_empty() {
        bail!("usage: sign.rs <artifact> [<artifact> ...]");
    }
    let key = signing_key(&release.name)?;
    ensure!(
        hex::encode(key.verifying_key().as_bytes())
            == include_str!("../../assets/update-key.pub").trim(),
        "the signing key does not match the public key embedded in the GUI"
    );
    for file in files {
        let bytes = read(&file).with_context(|| format!("read {file}"))?;
        let meta = Meta {
            size: bytes.len() as u64,
            sha256: hex::encode(Sha256::digest(&bytes)),
            sig: hex::encode(key.sign(&bytes).to_bytes()),
        };
        let out = format!("{file}.meta.json");
        write(&out, serde_json::to_string_pretty(&meta)?)?;
        println!("signed {file} -> {out}");
    }
    Ok(())
}

fn signing_key(name: &str) -> Result<SigningKey> {
    let env_name = format!("{}_UPDATE_KEY", name.to_uppercase().replace('-', "_"));
    let hex_key = match var(&env_name) {
        Ok(k) if !k.trim().is_empty() => k,
        _ => {
            let home = var("HOME").context("HOME not set")?;
            let path = format!("{home}/.config/{name}-hilen/update-key.hex");
            read_to_string(&path)
                .with_context(|| format!("{env_name} not set and {path} not found"))?
        }
    };
    let bytes = hex::decode(hex_key.trim()).context("update key is not hex")?;
    match SigningKey::try_from(bytes.as_slice()) {
        Ok(key) => Ok(key),
        Err(e) => bail!("update key: {e}"),
    }
}
