use aes_gcm::{Aes256Gcm, aead::KeyInit};
use base64::{engine::general_purpose, Engine};
use anyhow::{Result, anyhow};

pub fn build_cipher(key_b64: &str) -> Result<Aes256Gcm> {
    let key_bytes = general_purpose::STANDARD.decode(key_b64)
        .map_err(|e| anyhow!("Failed to decode base64 key: {}", e))?;

    Aes256Gcm::new_from_slice(&key_bytes)
        .map_err(|e| anyhow!("Invalid key length - must be exactly 32 bytes: {}", e))
}