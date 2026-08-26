use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::{bail, Context, Result};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::RngCore;
use sha2::{Digest, Sha256};

use crate::model::{EncryptedLedger, Ledger};

const AAD_PREFIX: &str = "token-monitor-ledger-v2:";

pub fn generate_key() -> String {
    let mut key = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut key);
    URL_SAFE_NO_PAD.encode(key)
}

pub fn decode_key(value: &str) -> Result<[u8; 32]> {
    let bytes = URL_SAFE_NO_PAD.decode(value.trim()).context("dashboard key is not valid base64url")?;
    if bytes.len() != 32 { bail!("dashboard key must decode to exactly 32 bytes"); }
    let mut key = [0u8; 32]; key.copy_from_slice(&bytes); Ok(key)
}

pub fn device_hash(device_id: &str) -> String {
    let mut hasher = Sha256::new(); hasher.update(device_id.as_bytes()); hex::encode(hasher.finalize())[..16].to_string()
}

pub fn encrypt_ledger(ledger: &Ledger, encoded_key: &str) -> Result<EncryptedLedger> {
    let key = decode_key(encoded_key)?; let cipher = Aes256Gcm::new_from_slice(&key).expect("AES-256 key length is fixed");
    let mut nonce_bytes = [0u8; 12]; rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);
    let hash = device_hash(&ledger.device.id); let aad = format!("{AAD_PREFIX}{hash}"); let plaintext = serde_json::to_vec(ledger)?;
    let ciphertext = cipher.encrypt(Nonce::from_slice(&nonce_bytes), Payload { msg: &plaintext, aad: aad.as_bytes() }).map_err(|_| anyhow::anyhow!("failed to encrypt ledger"))?;
    Ok(EncryptedLedger { schema_version:2,kind:"token-monitor-encrypted-ledger".to_string(),device_hash:hash,updated_at:ledger.generated_at.clone(),algorithm:"AES-256-GCM".to_string(),nonce:URL_SAFE_NO_PAD.encode(nonce_bytes),ciphertext:URL_SAFE_NO_PAD.encode(ciphertext) })
}

pub fn decrypt_ledger(envelope: &EncryptedLedger, encoded_key: &str) -> Result<Ledger> {
    let key=decode_key(encoded_key)?;let cipher=Aes256Gcm::new_from_slice(&key).expect("AES-256 key length is fixed");let nonce=URL_SAFE_NO_PAD.decode(&envelope.nonce)?;if nonce.len()!=12{bail!("invalid ledger nonce");}
    let ciphertext=URL_SAFE_NO_PAD.decode(&envelope.ciphertext)?;let aad=format!("{AAD_PREFIX}{}",envelope.device_hash);
    let plaintext=cipher.decrypt(Nonce::from_slice(&nonce),Payload{msg:&ciphertext,aad:aad.as_bytes()}).map_err(|_|anyhow::anyhow!("ledger authentication failed"))?;
    Ok(serde_json::from_slice(&plaintext)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DeviceInfo, Metrics};

    fn ledger() -> Ledger {
        Ledger { schema_version:2,generated_at:"2026-08-26T00:00:00Z".into(),device:DeviceInfo{id:"mac".into(),name:"Mac".into(),platform:"macos".into(),arch:"aarch64".into(),hostname:"mac".into(),app_version:"1".into()},rows:vec![],totals:Metrics{input:10,output:3,..Default::default()},scan_ms:5 }
    }

    #[test]
    fn round_trip() {
        let key=generate_key();let original=ledger();let encrypted=encrypt_ledger(&original,&key).unwrap();let decoded=decrypt_ledger(&encrypted,&key).unwrap();assert_eq!(decoded.totals.total_tokens(),13);assert_ne!(encrypted.ciphertext,serde_json::to_string(&original).unwrap());
    }

    #[test]
    fn wrong_key_fails_authentication() {
        let encrypted=encrypt_ledger(&ledger(),&generate_key()).unwrap();assert!(decrypt_ledger(&encrypted,&generate_key()).is_err());
    }
}
