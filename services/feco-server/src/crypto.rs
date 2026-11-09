//! Mã hóa dữ liệu nhạy cảm khi lưu (spec platform-security, "Mã hóa khi truyền và khi
//! lưu"): AES-256-GCM, nonce ngẫu nhiên 96 bit cho mỗi lần mã hóa.
//!
//! Định dạng: `[phiên bản khóa 1 byte][nonce 12 byte][bản mã + tag 16 byte]`. Dữ liệu
//! liên kết (AAD) gắn bản mã với ngữ cảnh của nó (ví dụ `webhook-secret:<client_id>`),
//! để không thể chép bản mã của dòng này sang dòng khác.
//!
//! Lớp này bảo vệ các cột nhạy cảm (khóa ký webhook của nhà phát triển, file xuất dữ
//! liệu cá nhân). Mã hóa toàn bộ đĩa của PostgreSQL và Kratos là việc của hạ tầng
//! staging/production (tasks.md 1.4).

use aes_gcm::{
    aead::{Aead, KeyInit, Payload},
    Aes256Gcm, Nonce,
};
use base64::Engine;

const VERSION: u8 = 1;
const NONCE_LEN: usize = 12;

#[derive(Clone)]
pub struct DataKey {
    cipher: Aes256Gcm,
}

impl std::fmt::Debug for DataKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DataKey(***)")
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CryptoError {
    #[error("khóa không hợp lệ (cần 32 byte)")]
    BadKey,
    #[error("bản mã hỏng, sai khóa hoặc sai ngữ cảnh")]
    Decrypt,
}

impl DataKey {
    pub fn from_bytes(key: &[u8]) -> Result<Self, CryptoError> {
        let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| CryptoError::BadKey)?;
        Ok(DataKey { cipher })
    }

    pub fn from_base64(key: &str) -> Result<Self, CryptoError> {
        let bytes = feco_config::decode_base64(key).ok_or(CryptoError::BadKey)?;
        Self::from_bytes(&bytes)
    }

    pub fn encrypt(&self, plaintext: &[u8], aad: &str) -> Vec<u8> {
        let mut nonce = [0u8; NONCE_LEN];
        getrandom::fill(&mut nonce).expect("nguồn ngẫu nhiên");
        let ct = self
            .cipher
            .encrypt(
                &Nonce::from(nonce),
                Payload {
                    msg: plaintext,
                    aad: aad.as_bytes(),
                },
            )
            .expect("AES-GCM không lỗi với độ dài hợp lệ");
        let mut out = Vec::with_capacity(1 + NONCE_LEN + ct.len());
        out.push(VERSION);
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&ct);
        out
    }

    pub fn decrypt(&self, data: &[u8], aad: &str) -> Result<Vec<u8>, CryptoError> {
        if data.len() < 1 + NONCE_LEN + 16 || data[0] != VERSION {
            return Err(CryptoError::Decrypt);
        }
        let (nonce, ct) = data[1..].split_at(NONCE_LEN);
        let nonce: [u8; NONCE_LEN] = nonce.try_into().map_err(|_| CryptoError::Decrypt)?;
        self.cipher
            .decrypt(
                &Nonce::from(nonce),
                Payload {
                    msg: ct,
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| CryptoError::Decrypt)
    }

    /// Mã hóa chuỗi thành văn bản lưu được trong cột `TEXT`: `enc:v1:<base64>`.
    pub fn encrypt_text(&self, plaintext: &str, aad: &str) -> String {
        let b64 = base64::engine::general_purpose::STANDARD_NO_PAD
            .encode(self.encrypt(plaintext.as_bytes(), aad));
        format!("enc:v1:{b64}")
    }

    pub fn decrypt_text(&self, stored: &str, aad: &str) -> Result<String, CryptoError> {
        let b64 = stored.strip_prefix("enc:v1:").ok_or(CryptoError::Decrypt)?;
        let data = base64::engine::general_purpose::STANDARD_NO_PAD
            .decode(b64)
            .map_err(|_| CryptoError::Decrypt)?;
        String::from_utf8(self.decrypt(&data, aad)?).map_err(|_| CryptoError::Decrypt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> DataKey {
        DataKey::from_bytes(&[7u8; 32]).unwrap()
    }

    #[test]
    fn round_trip_and_random_nonce() {
        let k = key();
        let a = k.encrypt(b"bi mat", "ctx");
        let b = k.encrypt(b"bi mat", "ctx");
        assert_ne!(a, b, "mỗi lần mã hóa dùng nonce khác nhau");
        assert_eq!(k.decrypt(&a, "ctx").unwrap(), b"bi mat");
        assert!(
            !a.windows(6).any(|w| w == b"bi mat"),
            "bản mã không chứa bản rõ"
        );
    }

    #[test]
    fn wrong_context_or_key_fails() {
        let k = key();
        let a = k.encrypt(b"bi mat", "webhook-secret:app-a");
        assert_eq!(
            k.decrypt(&a, "webhook-secret:app-b"),
            Err(CryptoError::Decrypt)
        );
        let other = DataKey::from_bytes(&[8u8; 32]).unwrap();
        assert_eq!(
            other.decrypt(&a, "webhook-secret:app-a"),
            Err(CryptoError::Decrypt)
        );
    }

    #[test]
    fn tampering_is_detected() {
        let k = key();
        let mut a = k.encrypt(b"bi mat", "ctx");
        let last = a.len() - 1;
        a[last] ^= 1;
        assert_eq!(k.decrypt(&a, "ctx"), Err(CryptoError::Decrypt));
    }

    #[test]
    fn text_form_round_trips() {
        let k = key();
        let s = k.encrypt_text("khoa-webhook", "ctx");
        assert!(s.starts_with("enc:v1:"));
        assert_eq!(k.decrypt_text(&s, "ctx").unwrap(), "khoa-webhook");
        assert!(
            k.decrypt_text("khoa-webhook", "ctx").is_err(),
            "bản rõ không được coi là bản mã"
        );
    }

    #[test]
    fn bad_keys_rejected_and_debug_hides_key() {
        assert_eq!(
            DataKey::from_bytes(&[1u8; 16]).unwrap_err(),
            CryptoError::BadKey
        );
        assert_eq!(format!("{:?}", key()), "DataKey(***)");
    }
}
