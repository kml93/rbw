use sha2::Digest as _;

const PHRASE_WORDS: usize = 5;

// https://github.com/bitwarden/sdk-internal/blob/main/crates/bitwarden-crypto/src/fingerprint.rs
/// The phrase Bitwarden clients show next to a public key, so that a user
/// can check that the key they are approving is the one this client sent.
/// For auth requests, `material` is the account email.
pub fn fingerprint_phrase(material: &str, public_key: &[u8]) -> String {
    let key_fingerprint = sha2::Sha256::digest(public_key);
    // unwrap is safe because a sha256 digest is a valid prk length
    let hkdf =
        hkdf::Hkdf::<sha2::Sha256>::from_prk(&key_fingerprint).unwrap();
    let mut number = [0_u8; 32];
    // unwrap is safe because 32 bytes is a valid output length for sha256
    hkdf.expand(material.as_bytes(), &mut number).unwrap();

    let words: Vec<&str> = (0..PHRASE_WORDS)
        .map(|_| crate::wordlist::EFF_LONG[div_rem_wordlist(&mut number)])
        .collect();
    words.join("-")
}

// Divides the big-endian number in place by the wordlist length and returns
// the remainder: this walks the hash as a base-7776 number, low digit first.
fn div_rem_wordlist(number: &mut [u8]) -> usize {
    let divisor = u32::try_from(crate::wordlist::EFF_LONG.len()).unwrap();
    let mut remainder = 0_u32;
    for byte in number.iter_mut() {
        let current = (remainder << 8) | u32::from(*byte);
        // unwrap is safe because remainder < divisor, so current / divisor
        // fits in a byte
        *byte = u8::try_from(current / divisor).unwrap();
        remainder = current % divisor;
    }
    // unwrap is safe because remainder < divisor, which is a usize length
    usize::try_from(remainder).unwrap()
}

#[test]
fn test_fingerprint_phrase() {
    // test vector from bitwarden/sdk-internal
    let public_key = [
        48, 130, 1, 34, 48, 13, 6, 9, 42, 134, 72, 134, 247, 13, 1, 1, 1, 5,
        0, 3, 130, 1, 15, 0, 48, 130, 1, 10, 2, 130, 1, 1, 0, 187, 38, 44,
        241, 110, 205, 89, 253, 25, 191, 126, 84, 121, 202, 61, 223, 189,
        244, 118, 212, 74, 139, 130, 97, 115, 164, 167, 106, 191, 188, 233,
        218, 196, 250, 187, 146, 125, 160, 150, 49, 198, 224, 176, 10, 0,
        143, 99, 230, 232, 160, 51, 104, 154, 211, 33, 80, 170, 4, 68, 80,
        219, 115, 167, 114, 156, 227, 125, 193, 128, 123, 39, 254, 191, 124,
        63, 129, 44, 63, 18, 56, 161, 48, 158, 0, 27, 146, 2, 99, 136, 75,
        21, 135, 6, 118, 12, 26, 251, 184, 172, 249, 53, 78, 210, 46, 143,
        17, 104, 202, 65, 173, 229, 219, 233, 144, 163, 101, 216, 238, 152,
        54, 158, 1, 195, 50, 203, 21, 226, 12, 82, 170, 175, 170, 160, 21,
        247, 248, 80, 97, 123, 0, 152, 116, 229, 126, 221, 199, 155, 194,
        192, 51, 207, 177, 240, 160, 84, 241, 41, 88, 176, 53, 111, 28, 173,
        177, 232, 158, 22, 79, 133, 152, 31, 32, 12, 196, 147, 58, 57, 50,
        252, 208, 131, 150, 179, 132, 178, 150, 234, 251, 143, 125, 163, 144,
        20, 46, 71, 168, 252, 164, 86, 120, 124, 56, 252, 206, 210, 236, 212,
        139, 127, 189, 236, 40, 46, 2, 238, 13, 216, 40, 48, 85, 133, 229,
        181, 155, 176, 217, 241, 154, 153, 213, 112, 222, 72, 219, 197, 3,
        219, 56, 77, 109, 47, 72, 251, 131, 36, 240, 96, 169, 31, 82, 93,
        166, 242, 3, 33, 213, 2, 3, 1, 0, 1,
    ];
    assert_eq!(
        fingerprint_phrase(
            "a09726a0-9590-49d1-a5f5-afe300b6a515",
            &public_key
        ),
        "turban-deftly-anime-chatroom-unselfish"
    );
}
