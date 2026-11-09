//! Định danh người dùng riêng cho từng app (spec user-data-consent, "Định danh riêng
//! cho từng app").
//!
//! `sub` mà app nhận = HMAC-SHA256(khóa bí mật, client_id ‖ 0x00 ‖ id danh tính), mã
//! base64url. Hai app khác nhau nhận hai giá trị khác nhau, không suy ngược được về
//! danh tính, và không ghép được với nhau. Giá trị này được Hydra đặt vào ID token
//! (`force_subject_identifier`) và được `/v1/me` trả về.

use base64::Engine;
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

#[derive(Clone)]
pub struct Pairwise {
    key: Vec<u8>,
}

impl std::fmt::Debug for Pairwise {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Pairwise(***)")
    }
}

impl Pairwise {
    pub fn new(secret: &str) -> Self {
        Pairwise {
            key: secret.as_bytes().to_vec(),
        }
    }

    pub fn subject(&self, client_id: &str, identity_id: &str) -> String {
        let mut mac = Hmac::<Sha256>::new_from_slice(&self.key).expect("HMAC nhận khóa mọi độ dài");
        mac.update(client_id.as_bytes());
        mac.update(&[0]);
        mac.update(identity_id.as_bytes());
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn differs_per_app_and_is_stable() {
        let p = Pairwise::new("khoa-bi-mat-thu-nghiem-dai-hon-32-ky-tu");
        let id = "11111111-2222-3333-4444-555555555555";
        let a = p.subject("soundlab", id);
        let b = p.subject("noteapp", id);
        assert_ne!(a, b);
        assert_eq!(a, p.subject("soundlab", id));
        assert_eq!(a.len(), 43);
        assert!(!a.contains(id));
    }

    #[test]
    fn separator_prevents_concatenation_collisions() {
        let p = Pairwise::new("k");
        assert_ne!(p.subject("ab", "c"), p.subject("a", "bc"));
    }

    #[test]
    fn debug_hides_key() {
        assert_eq!(format!("{:?}", Pairwise::new("secret")), "Pairwise(***)");
    }
}
