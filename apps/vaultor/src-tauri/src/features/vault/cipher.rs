use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng, Payload},
    Aes256Gcm, Key, Nonce,
};

use crate::error::VaultError;

/// Encrypt `plaintext` with AES-256-GCM.
///
/// `secret_id` is bound as Additional Authenticated Data (AAD): the ciphertext
/// can only be decrypted when the same `secret_id` is supplied. This prevents
/// moving ciphertext rows to a different secret without detection.
///
/// Returns `(ciphertext, nonce_bytes)`.
pub fn encrypt(
    key: &[u8; 32],
    secret_id: &str,
    plaintext: &str,
) -> Result<(Vec<u8>, Vec<u8>), VaultError> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

    let payload = Payload {
        msg: plaintext.as_bytes(),
        aad: secret_id.as_bytes(),
    };

    let ciphertext = cipher
        .encrypt(&nonce, payload)
        .map_err(|_| VaultError::Crypto("encryption failed".to_string()))?;

    Ok((ciphertext, nonce.to_vec()))
}

/// Decrypt `ciphertext` using `nonce` and `key`, verifying `secret_id` as AAD.
pub fn decrypt(
    key: &[u8; 32],
    secret_id: &str,
    ciphertext: &[u8],
    nonce: &[u8],
) -> Result<String, VaultError> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Nonce::from_slice(nonce);

    let payload = Payload {
        msg: ciphertext,
        aad: secret_id.as_bytes(),
    };

    let plaintext = cipher
        .decrypt(nonce, payload)
        .map_err(|_| VaultError::Crypto("decryption failed — data may be tampered".to_string()))?;

    String::from_utf8(plaintext)
        .map_err(|_| VaultError::Crypto("plaintext is not valid UTF-8".to_string()))
}

/// Encrypt raw bytes (for file secrets).
pub fn encrypt_bytes(
    key: &[u8; 32],
    secret_id: &str,
    plaintext: &[u8],
) -> Result<(Vec<u8>, Vec<u8>), VaultError> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

    let payload = Payload {
        msg: plaintext,
        aad: secret_id.as_bytes(),
    };

    let ciphertext = cipher
        .encrypt(&nonce, payload)
        .map_err(|_| VaultError::Crypto("encryption failed".to_string()))?;

    Ok((ciphertext, nonce.to_vec()))
}

/// Decrypt raw bytes (for file secrets).
pub fn decrypt_bytes(
    key: &[u8; 32],
    secret_id: &str,
    ciphertext: &[u8],
    nonce: &[u8],
) -> Result<Vec<u8>, VaultError> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Nonce::from_slice(nonce);

    let payload = Payload {
        msg: ciphertext,
        aad: secret_id.as_bytes(),
    };

    cipher
        .decrypt(nonce, payload)
        .map_err(|_| VaultError::Crypto("decryption failed — data may be tampered".to_string()))
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> [u8; 32] {
        [0x42u8; 32]
    }

    #[test]
    fn round_trip() {
        let key = test_key();
        let (ct, nonce) = encrypt(&key, "id-001", "hello world").unwrap();
        let plain = decrypt(&key, "id-001", &ct, &nonce).unwrap();
        assert_eq!(plain, "hello world");
    }

    #[test]
    fn wrong_key_fails() {
        let key = test_key();
        let (ct, nonce) = encrypt(&key, "id-001", "secret").unwrap();
        let bad_key = [0x00u8; 32];
        assert!(decrypt(&bad_key, "id-001", &ct, &nonce).is_err());
    }

    #[test]
    fn tampered_ciphertext_fails() {
        let key = test_key();
        let (mut ct, nonce) = encrypt(&key, "id-001", "secret").unwrap();
        ct[0] ^= 0xFF; // flip a bit
        assert!(decrypt(&key, "id-001", &ct, &nonce).is_err());
    }

    #[test]
    fn wrong_aad_fails() {
        let key = test_key();
        let (ct, nonce) = encrypt(&key, "id-001", "secret").unwrap();
        // Using a different secret_id as AAD should fail authentication
        assert!(decrypt(&key, "id-002", &ct, &nonce).is_err());
    }

    #[test]
    fn empty_plaintext() {
        let key = test_key();
        let (ct, nonce) = encrypt(&key, "id-001", "").unwrap();
        let plain = decrypt(&key, "id-001", &ct, &nonce).unwrap();
        assert_eq!(plain, "");
    }

    #[test]
    fn unicode_plaintext() {
        let key = test_key();
        let (ct, nonce) = encrypt(&key, "id-001", "🔐 bí mật").unwrap();
        let plain = decrypt(&key, "id-001", &ct, &nonce).unwrap();
        assert_eq!(plain, "🔐 bí mật");
    }
}
