//! Base64url without padding (RFC 4648 section 5), for the test headers.

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// Encodes `bytes` as unpadded base64url.
#[must_use]
pub fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..=chunk.len() {
            let index = ((n >> (18 - 6 * i)) & 63) as usize;
            out.push(char::from(ALPHABET[index]));
        }
    }
    out
}

/// Decodes base64url, padded or not. `None` when the text is not base64url.
#[must_use]
pub fn decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in text.trim_end_matches('=').bytes() {
        let v = ALPHABET.iter().position(|a| *a == c)? as u32;
        acc = ((acc << 6) | v) & 0xffff;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((acc >> bits) & 0xff) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_every_length() {
        for len in 0..9u8 {
            let bytes: Vec<u8> = (0..len).map(|i| i.wrapping_mul(97) ^ 0xfb).collect();
            assert_eq!(decode(&encode(&bytes)), Some(bytes));
        }
    }

    #[test]
    fn matches_the_rfc_vectors_and_uses_the_url_alphabet() {
        assert_eq!(encode(b"foobar"), "Zm9vYmFy");
        assert_eq!(encode(b"fo"), "Zm8");
        assert_eq!(encode(&[0xfb, 0xff]), "-_8");
        assert_eq!(decode("not base64!"), None);
    }
}
