use crate::error::VaultError;
use rand::seq::SliceRandom;
use rand::Rng;

const UPPERCASE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const LOWERCASE: &[u8] = b"abcdefghijklmnopqrstuvwxyz";
const DIGITS: &[u8] = b"0123456789";
const SYMBOLS: &[u8] = b"!@#$%^&*()-_=+[]{}|;:,.<>?";

/// Generate a cryptographically strong password with the given constraints.
///
/// At least one character set must be enabled and length must be 4–128.
/// The result is guaranteed to contain at least one character from each
/// selected character set.
#[tauri::command]
pub fn generate_password(
    length: u32,
    use_uppercase: bool,
    use_lowercase: bool,
    use_digits: bool,
    use_symbols: bool,
) -> Result<String, VaultError> {
    if !(4..=128).contains(&length) {
        return Err(VaultError::Validation(
            "Password length must be between 4 and 128".to_string(),
        ));
    }

    let charsets: Vec<&[u8]> = [
        (use_uppercase, UPPERCASE),
        (use_lowercase, LOWERCASE),
        (use_digits, DIGITS),
        (use_symbols, SYMBOLS),
    ]
    .iter()
    .filter(|(enabled, _)| *enabled)
    .map(|(_, cs)| *cs)
    .collect();

    if charsets.is_empty() {
        return Err(VaultError::Validation(
            "At least one character set must be selected".to_string(),
        ));
    }

    let mut rng = rand::thread_rng();

    // Guarantee at least one character from each selected charset.
    let mut password: Vec<u8> = charsets
        .iter()
        .map(|cs| cs[rng.gen_range(0..cs.len())])
        .collect();

    // Fill the remaining slots from the combined pool.
    let pool: Vec<u8> = charsets.iter().flat_map(|cs| cs.iter().copied()).collect();
    for _ in password.len()..(length as usize) {
        password.push(pool[rng.gen_range(0..pool.len())]);
    }

    // Shuffle so the guaranteed characters aren't always at the front.
    password.shuffle(&mut rng);

    Ok(String::from_utf8(password).expect("charset is ASCII"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_correct_length() {
        for len in [4, 16, 64, 128] {
            let pw = generate_password(len, true, true, true, true).unwrap();
            assert_eq!(pw.len(), len as usize, "expected length {len}");
        }
    }

    #[test]
    fn uppercase_only() {
        let pw = generate_password(20, true, false, false, false).unwrap();
        assert!(pw.chars().all(|c| c.is_ascii_uppercase()), "got: {pw}");
    }

    #[test]
    fn lowercase_only() {
        let pw = generate_password(20, false, true, false, false).unwrap();
        assert!(pw.chars().all(|c| c.is_ascii_lowercase()), "got: {pw}");
    }

    #[test]
    fn digits_only() {
        let pw = generate_password(20, false, false, true, false).unwrap();
        assert!(pw.chars().all(|c| c.is_ascii_digit()), "got: {pw}");
    }

    #[test]
    fn symbols_only() {
        let pw = generate_password(20, false, false, false, true).unwrap();
        let sym_set: Vec<char> = SYMBOLS.iter().map(|&b| b as char).collect();
        assert!(pw.chars().all(|c| sym_set.contains(&c)), "got: {pw}");
    }

    #[test]
    fn guarantees_each_selected_charset() {
        // Run multiple times to reduce flake probability.
        for _ in 0..20 {
            let pw = generate_password(20, true, true, true, true).unwrap();
            assert!(pw.chars().any(|c| c.is_ascii_uppercase()), "missing uppercase in: {pw}");
            assert!(pw.chars().any(|c| c.is_ascii_lowercase()), "missing lowercase in: {pw}");
            assert!(pw.chars().any(|c| c.is_ascii_digit()), "missing digit in: {pw}");
            let sym_set: Vec<char> = SYMBOLS.iter().map(|&b| b as char).collect();
            assert!(pw.chars().any(|c| sym_set.contains(&c)), "missing symbol in: {pw}");
        }
    }

    #[test]
    fn rejects_zero_charsets() {
        let err = generate_password(16, false, false, false, false).unwrap_err();
        assert!(err.to_string().contains("character set"));
    }

    #[test]
    fn rejects_length_too_short() {
        let err = generate_password(3, true, true, true, true).unwrap_err();
        assert!(err.to_string().contains("length"));
    }

    #[test]
    fn rejects_length_too_long() {
        let err = generate_password(129, true, true, true, true).unwrap_err();
        assert!(err.to_string().contains("length"));
    }

    #[test]
    fn rejects_length_zero() {
        let err = generate_password(0, true, true, true, true).unwrap_err();
        assert!(err.to_string().contains("length"));
    }

    #[test]
    fn min_length_with_all_charsets() {
        // length=4, 4 charsets → exactly one guaranteed char per charset, no extras
        let pw = generate_password(4, true, true, true, true).unwrap();
        assert_eq!(pw.len(), 4);
        assert!(pw.chars().any(|c| c.is_ascii_uppercase()));
        assert!(pw.chars().any(|c| c.is_ascii_lowercase()));
        assert!(pw.chars().any(|c| c.is_ascii_digit()));
    }
}
