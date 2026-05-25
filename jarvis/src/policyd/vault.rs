/// AES-256-GCM credential vault.
///
/// Security contract (Architecture doc, Section 9):
/// - Credentials NEVER appear in LLM prompts, audit logs, or evolution diffs.
/// - The only way credentials leave the vault is via Command::env(key, value) at exec boundary.
/// - vault.key and vault.enc are in blocked_paths — never read by the agent.
/// - Key is derived from a user passphrase (PBKDF2-HMAC-SHA256, 100k iterations) or
///   from a random key stored in vault.key (automated / no-passphrase mode).

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use aes_gcm::aead::rand_core::RngCore;
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{debug, info};

const NONCE_LEN: usize = 12;
const KEY_LEN: usize   = 32;

#[derive(Debug, Serialize, Deserialize)]
struct VaultStore {
    entries: HashMap<String, EncryptedEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
struct EncryptedEntry {
    /// base64-encoded nonce (12 bytes)
    nonce: String,
    /// base64-encoded ciphertext
    ciphertext: String,
}

pub struct CredentialVault {
    key: [u8; KEY_LEN],
    vault_path: PathBuf,
}

impl CredentialVault {
    /// Open or create vault. Key file is auto-generated on first use.
    /// In production: replace with passphrase-derived key (PBKDF2).
    pub fn open(data_dir: &Path) -> Result<Self> {
        let key_path   = data_dir.join("vault.key");
        let vault_path = data_dir.join("vault.enc");

        let key = if key_path.exists() {
            let raw = std::fs::read(&key_path)?;
            if raw.len() != KEY_LEN {
                bail!("vault.key has wrong length — delete and restart to regenerate");
            }
            let mut k = [0u8; KEY_LEN];
            k.copy_from_slice(&raw);
            k
        } else {
            let mut k = [0u8; KEY_LEN];
            OsRng.fill_bytes(&mut k);
            std::fs::write(&key_path, &k)?;
            // Restrict permissions to owner-read-only
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600))?;
            }
            info!("vault: generated new key at {}", key_path.display());
            k
        };

        Ok(Self { key, vault_path })
    }

    /// Store a credential. Immediately encrypted and persisted.
    pub fn set(&self, name: &str, value: &str) -> Result<()> {
        let mut store = self.load_store()?;

        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.key));
        let mut nonce_bytes = [0u8; NONCE_LEN];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher.encrypt(nonce, value.as_bytes())
            .map_err(|e| anyhow::anyhow!("vault encrypt: {e}"))?;

        store.entries.insert(name.to_string(), EncryptedEntry {
            nonce: base64_encode(&nonce_bytes),
            ciphertext: base64_encode(&ciphertext),
        });

        self.save_store(&store)?;
        debug!("vault: stored credential '{name}'");
        Ok(())
    }

    /// Retrieve a plaintext credential. Returns None if not found.
    pub fn get(&self, name: &str) -> Result<Option<String>> {
        let store = self.load_store()?;
        let Some(entry) = store.entries.get(name) else {
            return Ok(None);
        };

        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.key));
        let nonce_bytes = base64_decode(&entry.nonce)?;
        let ciphertext  = base64_decode(&entry.ciphertext)?;

        if nonce_bytes.len() != NONCE_LEN {
            bail!("vault: corrupt nonce for '{name}'");
        }
        let nonce = Nonce::from_slice(&nonce_bytes);
        let plaintext = cipher.decrypt(nonce, ciphertext.as_slice())
            .map_err(|_| anyhow::anyhow!("vault: decryption failed for '{name}' — wrong key?"))?;

        Ok(Some(String::from_utf8(plaintext)?))
    }

    /// Delete a credential.
    pub fn remove(&self, name: &str) -> Result<bool> {
        let mut store = self.load_store()?;
        let removed = store.entries.remove(name).is_some();
        if removed {
            self.save_store(&store)?;
        }
        Ok(removed)
    }

    pub fn list_names(&self) -> Result<Vec<String>> {
        let store = self.load_store()?;
        let mut names: Vec<_> = store.entries.keys().cloned().collect();
        names.sort();
        Ok(names)
    }

    /// Inject named credentials as env vars on a subprocess Command.
    /// This is the ONLY approved path for credentials to leave the vault.
    pub fn inject_env(&self, cmd: &mut std::process::Command, names: &[&str]) -> Result<()> {
        for name in names {
            if let Some(value) = self.get(name)? {
                cmd.env(name.to_uppercase().replace('-', "_"), value);
            }
        }
        Ok(())
    }

    fn load_store(&self) -> Result<VaultStore> {
        if !self.vault_path.exists() {
            return Ok(VaultStore { entries: HashMap::new() });
        }
        let bytes = std::fs::read(&self.vault_path)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    fn save_store(&self, store: &VaultStore) -> Result<()> {
        let bytes = serde_json::to_vec(store)?;
        std::fs::write(&self.vault_path, bytes)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&self.vault_path, std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(())
    }
}

fn base64_encode(data: &[u8]) -> String {
    
    // Simple base64 using hex as a stand-in — real base64 via manual impl
    // to avoid adding another dep. In production replace with base64 crate.
    engine_encode(data)
}

fn base64_decode(s: &str) -> Result<Vec<u8>> {
    engine_decode(s).ok_or_else(|| anyhow::anyhow!("invalid base64"))
}

// Minimal base64 implementation (avoids adding base64 crate for now).
// RFC 4648 standard alphabet.
fn engine_encode(input: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let combined = (b0 << 16) | (b1 << 8) | b2;
        out.push(CHARS[((combined >> 18) & 0x3f) as usize] as char);
        out.push(CHARS[((combined >> 12) & 0x3f) as usize] as char);
        out.push(if chunk.len() > 1 { CHARS[((combined >> 6) & 0x3f) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { CHARS[(combined & 0x3f) as usize] as char } else { '=' });
    }
    out
}

fn engine_decode(s: &str) -> Option<Vec<u8>> {
    const INV: [i8; 128] = {
        let mut t = [-1i8; 128];
        let chars = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut i = 0usize;
        while i < chars.len() { t[chars[i] as usize] = i as i8; i += 1; }
        t
    };

    let bytes: Vec<u8> = s.bytes().filter(|&b| b != b'=').collect();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);

    for chunk in bytes.chunks(4) {
        let vals: Vec<u8> = chunk.iter().map(|&b| {
            if b >= 128 { return 255u8; }
            let v = INV[b as usize];
            if v < 0 { 255u8 } else { v as u8 }
        }).collect();
        if vals.iter().any(|&v| v == 255) { return None; }
        let b0 = (vals[0] << 2) | (vals[1] >> 4);
        out.push(b0);
        if chunk.len() > 2 {
            out.push((vals[1] << 4) | (vals[2] >> 2));
        }
        if chunk.len() > 3 {
            out.push((vals[2] << 6) | vals[3]);
        }
    }
    Some(out)
}
