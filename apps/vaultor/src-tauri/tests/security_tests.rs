//! Security and memory-dump tests for Vaultor.
//!
//! These tests verify that:
//! 1. The app does not crash under memory-pressure scenarios (large payloads,
//!    rapid session cycling, concurrent access).
//! 2. Key material is not accessible via API after session invalidation.
//! 3. Cryptographic invariants hold (nonce uniqueness, AAD binding, tamper
//!    detection).
//! 4. Malformed / adversarial inputs are rejected without panicking.

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use vaultor_lib::features::auth::session::SessionStore;
use vaultor_lib::features::keychain::MockKeyProvider;
use vaultor_lib::features::vault::cipher;

// ── Memory-dump crash tests ────────────────────────────────────────────────

/// Simulate a "memory dump" scenario: allocate, encrypt, decrypt, and drop
/// large payloads in a tight loop. The test passes if the process survives
/// without panicking or OOM-killing.
#[test]
fn memory_dump_large_payload_no_crash() {
    let key = [0x42u8; 32];
    let secret_id = "stress-large";

    // 1 MB plaintext — repeated encrypt / decrypt cycles.
    let large_plaintext: String = "A".repeat(1_048_576);

    for round in 0..5 {
        let (ct, nonce) = cipher::encrypt(&key, secret_id, &large_plaintext)
            .unwrap_or_else(|e| panic!("encrypt round {round} failed: {e}"));
        let decrypted = cipher::decrypt(&key, secret_id, &ct, &nonce)
            .unwrap_or_else(|e| panic!("decrypt round {round} failed: {e}"));
        assert_eq!(decrypted.len(), large_plaintext.len());
    }
}

/// Encrypt/decrypt large binary blobs (file secrets path) without crashing.
#[test]
fn memory_dump_large_binary_no_crash() {
    let key = [0x42u8; 32];
    let secret_id = "stress-binary";

    // 1 MB of random-ish binary data.
    let large_binary: Vec<u8> = (0u8..=255).cycle().take(1_048_576).collect();

    for round in 0..5 {
        let (ct, nonce) = cipher::encrypt_bytes(&key, secret_id, &large_binary)
            .unwrap_or_else(|e| panic!("encrypt_bytes round {round} failed: {e}"));
        let decrypted = cipher::decrypt_bytes(&key, secret_id, &ct, &nonce)
            .unwrap_or_else(|e| panic!("decrypt_bytes round {round} failed: {e}"));
        assert_eq!(decrypted, large_binary);
    }
}

/// Rapidly create and invalidate sessions to simulate memory churn.
/// Verifies that repeated alloc / zeroize cycles don't crash.
#[test]
fn memory_dump_rapid_session_cycling_no_crash() {
    let store = SessionStore::new();

    for i in 0..1000 {
        let key = [(i & 0xFF) as u8; 32];
        store.create(key, Some(Duration::from_millis(50)));
        assert!(store.is_valid(), "session {i} should be valid immediately");
        store.invalidate();
        assert!(
            !store.is_valid(),
            "session {i} should be invalid after lock"
        );
    }
}

/// Interleave encryption with session create/destroy to simulate real
/// workload churn. Must not crash.
#[test]
fn memory_dump_interleaved_crypto_and_sessions() {
    let store = SessionStore::new();
    let key_bytes = [0xAB; 32];

    for i in 0..200 {
        store.create(key_bytes, Some(Duration::from_secs(60)));

        let retrieved = store.get_key().unwrap();
        let key: [u8; 32] = retrieved.try_into().unwrap();

        let plaintext = format!("secret-value-{i}");
        let (ct, nonce) = cipher::encrypt(&key, "sid", &plaintext).unwrap();
        let decrypted = cipher::decrypt(&key, "sid", &ct, &nonce).unwrap();
        assert_eq!(decrypted, plaintext);

        store.invalidate();
    }
}

// ── Key-material zeroization contract tests ────────────────────────────────

/// After invalidation the encryption key must not be retrievable.
#[test]
fn key_not_retrievable_after_invalidation() {
    let store = SessionStore::new();
    let key = [0xDE; 32];
    store.create(key, Some(Duration::from_secs(120)));
    assert_eq!(store.get_key().unwrap(), key.to_vec());

    store.invalidate();
    assert!(
        store.get_key().is_err(),
        "key must not be accessible after invalidation"
    );
    let status = store.status();
    assert!(!status.active);
    assert!(status.expires_at_ms.is_none());
}

/// Replacing a session must make the old key unretrievable.
#[test]
fn old_key_replaced_on_new_session() {
    let store = SessionStore::new();
    let key_a = [0xAA; 32];
    let key_b = [0xBB; 32];

    store.create(key_a, Some(Duration::from_secs(120)));
    assert_eq!(store.get_key().unwrap(), key_a.to_vec());

    store.create(key_b, Some(Duration::from_secs(120)));
    let retrieved = store.get_key().unwrap();
    assert_eq!(retrieved, key_b.to_vec(), "new key must replace old key");
    assert_ne!(retrieved, key_a.to_vec(), "old key must not be retrievable");
}

/// After zero-TTL expiry the key must not be accessible.
#[test]
fn key_inaccessible_after_ttl_expiry() {
    let store = SessionStore::new();
    store.create([0xCC; 32], Some(Duration::from_secs(0)));
    thread::sleep(Duration::from_millis(20));
    assert!(
        store.get_key().is_err(),
        "key must not be accessible after TTL expires"
    );
}

// ── Cryptographic invariant tests ──────────────────────────────────────────

/// Every call to encrypt must produce a unique nonce (birthday resistance).
#[test]
fn nonce_uniqueness_across_encryptions() {
    let key = [0x42u8; 32];
    let mut nonces: Vec<Vec<u8>> = Vec::new();

    for _ in 0..500 {
        let (_, nonce) = cipher::encrypt(&key, "nonce-test", "same plaintext").unwrap();
        assert_eq!(nonce.len(), 12, "AES-256-GCM nonce must be 12 bytes");
        assert!(
            !nonces.contains(&nonce),
            "duplicate nonce detected — catastrophic for GCM security"
        );
        nonces.push(nonce);
    }
}

/// Same plaintext + same key must produce different ciphertext each time
/// (due to random nonce).
#[test]
fn same_plaintext_different_ciphertext() {
    let key = [0x42u8; 32];
    let (ct1, _) = cipher::encrypt(&key, "id", "same").unwrap();
    let (ct2, _) = cipher::encrypt(&key, "id", "same").unwrap();
    assert_ne!(ct1, ct2, "identical ciphertext would indicate nonce reuse");
}

/// Tampered ciphertext must be rejected.
#[test]
fn tampered_ciphertext_detected() {
    let key = [0x42u8; 32];
    let (mut ct, nonce) = cipher::encrypt(&key, "id", "secret").unwrap();
    // Flip every byte position and verify each is caught.
    for pos in 0..ct.len() {
        ct[pos] ^= 0xFF;
        assert!(
            cipher::decrypt(&key, "id", &ct, &nonce).is_err(),
            "tamper at position {pos} was not detected"
        );
        ct[pos] ^= 0xFF; // restore
    }
}

/// Tampered nonce must be rejected.
#[test]
fn tampered_nonce_detected() {
    let key = [0x42u8; 32];
    let (ct, mut nonce) = cipher::encrypt(&key, "id", "secret").unwrap();
    nonce[0] ^= 0x01;
    assert!(
        cipher::decrypt(&key, "id", &ct, &nonce).is_err(),
        "tampered nonce was not detected"
    );
}

/// Truncated ciphertext must not cause a panic.
#[test]
fn truncated_ciphertext_no_panic() {
    let key = [0x42u8; 32];
    let (ct, nonce) = cipher::encrypt(&key, "id", "secret").unwrap();

    for truncate_to in 0..ct.len() {
        let truncated = &ct[..truncate_to];
        // Must return Err, never panic.
        let _ = cipher::decrypt(&key, "id", truncated, &nonce);
    }
}

/// Empty ciphertext must be rejected, not panic.
#[test]
fn empty_ciphertext_rejected() {
    let key = [0x42u8; 32];
    let nonce = [0u8; 12];
    assert!(cipher::decrypt(&key, "id", &[], &nonce).is_err());
}

/// AAD mismatch must be detected on binary (file) secrets too.
#[test]
fn aad_mismatch_on_binary_secrets() {
    let key = [0x42u8; 32];
    let data = b"binary file content";
    let (ct, nonce) = cipher::encrypt_bytes(&key, "file-1", data).unwrap();
    assert!(
        cipher::decrypt_bytes(&key, "file-2", &ct, &nonce).is_err(),
        "AAD mismatch must be caught for binary secrets"
    );
}

/// Binary encrypt/decrypt round-trip with all-zero payload.
#[test]
fn binary_all_zeros_roundtrip() {
    let key = [0x42u8; 32];
    let zeros = vec![0u8; 4096];
    let (ct, nonce) = cipher::encrypt_bytes(&key, "zero-test", &zeros).unwrap();
    let decrypted = cipher::decrypt_bytes(&key, "zero-test", &ct, &nonce).unwrap();
    assert_eq!(decrypted, zeros);
}

/// Binary encrypt/decrypt round-trip with 0xFF payload.
#[test]
fn binary_all_ones_roundtrip() {
    let key = [0x42u8; 32];
    let ones = vec![0xFFu8; 4096];
    let (ct, nonce) = cipher::encrypt_bytes(&key, "ones-test", &ones).unwrap();
    let decrypted = cipher::decrypt_bytes(&key, "ones-test", &ct, &nonce).unwrap();
    assert_eq!(decrypted, ones);
}

// ── Concurrent access safety ───────────────────────────────────────────────

/// Multiple threads reading the session while another invalidates it.
/// Must never panic, deadlock, or return key material after invalidation
/// has completed.
#[test]
fn concurrent_session_access_no_crash() {
    let store = Arc::new(SessionStore::new());
    store.create([0xEE; 32], Some(Duration::from_secs(60)));

    let mut handles = Vec::new();

    // Spawn 8 reader threads.
    for _ in 0..8 {
        let s = Arc::clone(&store);
        handles.push(thread::spawn(move || {
            for _ in 0..500 {
                let _ = s.is_valid();
                let _ = s.get_key();
                let _ = s.status();
            }
        }));
    }

    // Spawn 2 writer threads that cycle sessions.
    for t in 0..2 {
        let s = Arc::clone(&store);
        handles.push(thread::spawn(move || {
            for i in 0..100 {
                let k = [((t * 100 + i) & 0xFF) as u8; 32];
                s.create(k, Some(Duration::from_secs(60)));
                thread::yield_now();
                s.invalidate();
            }
        }));
    }

    for h in handles {
        h.join().expect("thread must not panic");
    }

    // After all writers are done and have invalidated, session should be gone.
    assert!(!store.is_valid());
}

/// Concurrent encryption operations must not interfere with each other.
#[test]
fn concurrent_encryption_no_crash() {
    let key = [0x42u8; 32];
    let mut handles = Vec::new();

    for t in 0..8 {
        handles.push(thread::spawn(move || {
            for i in 0..100 {
                let plaintext = format!("thread-{t}-round-{i}");
                let sid = format!("concurrent-{t}");
                let (ct, nonce) = cipher::encrypt(&key, &sid, &plaintext).unwrap();
                let decrypted = cipher::decrypt(&key, &sid, &ct, &nonce).unwrap();
                assert_eq!(decrypted, plaintext);
            }
        }));
    }

    for h in handles {
        h.join().expect("encryption thread must not panic");
    }
}

// ── Adversarial input tests ────────────────────────────────────────────────

/// Very long secret_id (AAD) must not crash.
#[test]
fn very_long_aad_no_crash() {
    let key = [0x42u8; 32];
    let long_id = "x".repeat(100_000);
    let (ct, nonce) = cipher::encrypt(&key, &long_id, "value").unwrap();
    let decrypted = cipher::decrypt(&key, &long_id, &ct, &nonce).unwrap();
    assert_eq!(decrypted, "value");
}

/// Empty secret_id (AAD) is valid and must round-trip.
#[test]
fn empty_aad_roundtrip() {
    let key = [0x42u8; 32];
    let (ct, nonce) = cipher::encrypt(&key, "", "value").unwrap();
    let decrypted = cipher::decrypt(&key, "", &ct, &nonce).unwrap();
    assert_eq!(decrypted, "value");
}

/// Very long plaintext (4 MB) must not crash.
#[test]
fn very_large_plaintext_no_crash() {
    let key = [0x42u8; 32];
    let big = "B".repeat(4 * 1024 * 1024);
    let (ct, nonce) = cipher::encrypt(&key, "big", &big).unwrap();
    let decrypted = cipher::decrypt(&key, "big", &ct, &nonce).unwrap();
    assert_eq!(decrypted.len(), big.len());
}

/// Multi-byte UTF-8 characters must survive encryption round-trip.
#[test]
fn multibyte_utf8_roundtrip() {
    let key = [0x42u8; 32];
    let texts = [
        "日本語テスト",
        "🔐🗝️🔑",
        "Ω≈ç√∫",
        "\u{0000}null\u{0000}bytes",
        "mixed 中文 and English",
    ];
    for text in &texts {
        let (ct, nonce) = cipher::encrypt(&key, "utf8", text).unwrap();
        let decrypted = cipher::decrypt(&key, "utf8", &ct, &nonce).unwrap();
        assert_eq!(&decrypted, text, "UTF-8 round-trip failed for: {text}");
    }
}

// ── MockKeyProvider tests ──────────────────────────────────────────────────

/// MockKeyProvider must be deterministic (same key every call).
#[test]
fn mock_key_provider_deterministic() {
    use vaultor_lib::features::keychain::KeyProvider;

    let provider = MockKeyProvider::default();
    let k1 = provider.get_or_create_key().unwrap();
    let k2 = provider.get_or_create_key().unwrap();
    assert_eq!(k1, k2, "MockKeyProvider must return the same key");
}

// ── Password generator security tests ──────────────────────────────────────

/// Generated passwords must contain at least one char from each enabled set.
/// Run with high iteration count to catch probabilistic failures.
#[test]
fn password_generator_charset_guarantee() {
    use vaultor_lib::commands::generate::generate_password;

    for _ in 0..100 {
        let pw = generate_password(16, true, true, true, true).unwrap();
        assert!(pw.chars().any(|c| c.is_ascii_uppercase()), "no uppercase");
        assert!(pw.chars().any(|c| c.is_ascii_lowercase()), "no lowercase");
        assert!(pw.chars().any(|c| c.is_ascii_digit()), "no digit");
        assert!(pw.chars().any(|c| !c.is_ascii_alphanumeric()), "no symbol");
    }
}

/// Generated passwords at different lengths must match requested length.
#[test]
fn password_generator_exact_length() {
    use vaultor_lib::commands::generate::generate_password;

    for len in [4, 8, 16, 32, 64, 128] {
        let pw = generate_password(len, true, true, true, true).unwrap();
        assert_eq!(pw.len(), len as usize);
    }
}

/// Boundary lengths must not crash.
#[test]
fn password_generator_boundary_lengths() {
    use vaultor_lib::commands::generate::generate_password;

    // Valid boundaries
    assert!(generate_password(4, true, true, true, true).is_ok());
    assert!(generate_password(128, true, true, true, true).is_ok());

    // Invalid boundaries
    assert!(generate_password(3, true, true, true, true).is_err());
    assert!(generate_password(129, true, true, true, true).is_err());
    assert!(generate_password(0, true, true, true, true).is_err());
    assert!(generate_password(u32::MAX, true, true, true, true).is_err());
}
