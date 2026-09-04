use std::time::{SystemTime, UNIX_EPOCH};
use aes::Aes256;
use base64::prelude::*;
use cbc::cipher::{block_padding::Pkcs7, BlockEncryptMut, KeyIvInit};
use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};
use rand::Rng;
use sha2::{Digest, Sha256};
use url::Url;

pub const DEFAULT_SECRET_PREFIX: &str = "4cjLaD4jGRwlQ9U72xIzEBe0vHBmf9";
const NONCE_CHARS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";

const QUERY_ENCODE_SET: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'<')
    .add(b'>')
    .add(b'`')
    .add(b'?')
    .add(b'{')
    .add(b'}')
    .add(b'/')
    .add(b':')
    .add(b';')
    .add(b'=')
    .add(b'@')
    .add(b'[')
    .add(b'\\')
    .add(b']')
    .add(b'^')
    .add(b'|')
    .add(b'+')
    .add(b',')
    .add(b'$')
    .add(b'&')
    .add(b'%');

type Aes256CbcEnc = cbc::Encryptor<Aes256>;

#[derive(Clone, Debug)]
pub struct AppleAuthenticator {
    secret_prefix: String,
    session_id: String,
}

impl Default for AppleAuthenticator {
    fn default() -> Self {
        Self::new(DEFAULT_SECRET_PREFIX, None)
    }
}

impl AppleAuthenticator {
    pub fn new(secret_prefix: &str, session_id: Option<String>) -> Self {
        let session_id = session_id.unwrap_or_else(Self::generate_session_id);
        Self {
            secret_prefix: secret_prefix.to_string(),
            session_id,
        }
    }

    pub fn generate_session_id() -> String {
        let mut rng = rand::thread_rng();
        (0..40)
            .map(|_| {
                let digit: u8 = rng.gen_range(0..10);
                (b'0' + digit) as char
            })
            .collect()
    }

    pub fn generate_nonce(length: usize) -> String {
        let mut rng = rand::thread_rng();
        (0..length)
            .map(|_| {
                let idx = rng.gen_range(0..NONCE_CHARS.len());
                NONCE_CHARS[idx] as char
            })
            .collect()
    }

    pub fn authenticate_url(&self, raw_url: &str, expiry_seconds: u64) -> Result<String, String> {
        let nonce = Self::generate_nonce(16);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_secs();
        let expiry = now + expiry_seconds;

        self.authenticate_url_with_params(raw_url, &nonce, expiry)
    }

    pub fn authenticate_url_with_params(
        &self,
        raw_url: &str,
        nonce: &str,
        expiry: u64,
    ) -> Result<String, String> {
        let parsed = Url::parse(raw_url).map_err(|e| e.to_string())?;

        let has_query = parsed.query().is_some();
        let sep = if has_query { "&" } else { "?" };

        let mut path_and_query = parsed.path().to_string();
        if let Some(query) = parsed.query() {
            path_and_query.push('?');
            path_and_query.push_str(query);
        }

        let plaintext = format!(
            "{path_and_query}{sep}sid={}{expiry}{nonce}",
            self.session_id
        );

        let combined_secret = format!("{}{nonce}", self.secret_prefix);
        let mut hasher = Sha256::new();
        hasher.update(combined_secret.as_bytes());
        let key_hash = hasher.finalize();

        let iv = [0u8; 16];

        let cipher = Aes256CbcEnc::new_from_slices(&key_hash, &iv)
            .map_err(|e| format!("Cipher init error: {e}"))?;

        let pt_bytes = plaintext.as_bytes();
        let msg_len = pt_bytes.len();
        let mut buf = vec![0u8; msg_len + 16];
        buf[..msg_len].copy_from_slice(pt_bytes);

        let encrypted = cipher
            .encrypt_padded_mut::<Pkcs7>(&mut buf, msg_len)
            .map_err(|e| format!("Encryption error: {e}"))?;
        let b64_cipher = BASE64_STANDARD.encode(encrypted);
        let quoted_b64 = utf8_percent_encode(&b64_cipher, QUERY_ENCODE_SET).to_string();

        let access_key = format!("{expiry}_{nonce}_{quoted_b64}");
        let authenticated_url = format!("{raw_url}{sep}sid={}&accessKey={access_key}", self.session_id);

        Ok(authenticated_url)
    }

    #[allow(dead_code)]
    pub fn session_id(&self) -> &str {
        &self.session_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deterministic_signing_vector() {
        let auth = AppleAuthenticator::new(
            DEFAULT_SECRET_PREFIX,
            Some("1234567890123456789012345678901234567890".to_string()),
        );
        let url = "https://sat-cdn.apple-mapkit.com/tile?style=7&size=2&scale=2&z=3&x=2&y=1&v=10421";
        let nonce = "ABCDEFGHIJKLMNOP";
        let expiry = 1700000000;

        let signed = auth.authenticate_url_with_params(url, nonce, expiry).unwrap();

        let expected_quoted_b64 = "LYko9JJtEcAmX0KrqN2e6dxuGGuxl6UN4uWdiB71cUQkmOGB5t9zqsNHYYxh4UiU7eWp%2BKc9rIVBCOML0ewjMqNbpgV7HxkOyWWg%2BHtRvZgeMM9WWk0%2BZgMV69KAXOnXtQJLsI7%2BXcfgjjVC4QPrDGcRFeGUIK0W530ifF4SAhE%3D";
        let expected_access_key = format!("1700000000_ABCDEFGHIJKLMNOP_{expected_quoted_b64}");
        let expected_url = format!("{url}&sid=1234567890123456789012345678901234567890&accessKey={expected_access_key}");

        assert_eq!(signed, expected_url);
    }

    #[test]
    fn test_session_id_format() {
        let sid = AppleAuthenticator::generate_session_id();
        assert_eq!(sid.len(), 40);
        assert!(sid.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn test_nonce_format() {
        let nonce = AppleAuthenticator::generate_nonce(16);
        assert_eq!(nonce.len(), 16);
        assert!(nonce.chars().all(|c| c.is_ascii_alphanumeric()));
    }
}
