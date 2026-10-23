//! DB 에 넣는 비밀값 암호화(운영자 OTP 비밀값)와 서버 비밀 해시(인증 코드).
//!
//! - AES-256-GCM. 저장 형식: `enc:v1:` + base64(nonce 12바이트 ‖ 암호문)
//! - 키: 환경변수 `DATA_KEY` (`openssl rand -hex 32` 로 만든 64자).
//!   없으면 `DATABASE_URL` 에서 파생합니다 (DB 주소·비밀번호를 바꾸면 저장된 OTP 를 다시 등록해야 하므로
//!   운영에서는 `DATA_KEY` 를 따로 두세요).

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use hmac::{Hmac, Mac};
use rand::RngCore;
use sha2::{Digest, Sha256};

const PREFIX: &str = "enc:v1:";

#[derive(Clone)]
pub struct SecretBox {
    cipher: Aes256Gcm,
    mac_key: [u8; 32],
}

impl std::fmt::Debug for SecretBox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretBox(..)")
    }
}

impl SecretBox {
    pub fn from_config(data_key: Option<&str>, fallback_seed: &str) -> Result<Self, String> {
        let key: [u8; 32] = match data_key.map(str::trim).filter(|k| !k.is_empty()) {
            Some(hexkey) => hex::decode(hexkey)
                .ok()
                .and_then(|b| b.try_into().ok())
                .ok_or_else(|| "DATA_KEY 는 16진수 64자여야 합니다 (openssl rand -hex 32)".to_string())?,
            None => {
                let mut h = Sha256::new();
                h.update(b"gageyo-data-key-v1:");
                h.update(fallback_seed.as_bytes());
                h.finalize().into()
            }
        };
        let mut m = Sha256::new();
        m.update(b"gageyo-mac-key-v1:");
        m.update(key);
        Ok(Self { cipher: Aes256Gcm::new(&key.into()), mac_key: m.finalize().into() })
    }

    pub fn encrypt(&self, plain: &str) -> String {
        if plain.is_empty() {
            return String::new();
        }
        let mut nonce = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce);
        let ct = self.cipher.encrypt(Nonce::from_slice(&nonce), plain.as_bytes()).expect("aes-gcm encrypt");
        let mut buf = nonce.to_vec();
        buf.extend_from_slice(&ct);
        format!("{PREFIX}{}", B64.encode(buf))
    }

    /// 암호문이면 풀고, 평문이면 그대로. 풀 수 없으면 Err (키가 바뀜 등)
    pub fn decrypt(&self, stored: &str) -> Result<String, String> {
        let Some(b64) = stored.strip_prefix(PREFIX) else { return Ok(stored.to_string()) };
        let raw = B64.decode(b64).map_err(|_| "암호문 형식 오류".to_string())?;
        if raw.len() < 13 {
            return Err("암호문 형식 오류".into());
        }
        let (nonce, ct) = raw.split_at(12);
        let pt = self
            .cipher
            .decrypt(Nonce::from_slice(nonce), ct)
            .map_err(|_| "비밀값을 풀 수 없습니다 (DATA_KEY 가 바뀌었는지 확인)".to_string())?;
        String::from_utf8(pt).map_err(|_| "암호문 형식 오류".into())
    }

    /// 서버 비밀키로 만든 HMAC-SHA256 (16진수). 인증 코드처럼 짧은 값을 DB 에 해시로 둘 때 씀
    pub fn keyed_hash(&self, parts: &[&str]) -> String {
        let mut mac = Hmac::<Sha256>::new_from_slice(&self.mac_key).expect("hmac key");
        for (i, p) in parts.iter().enumerate() {
            if i > 0 {
                mac.update(b"|");
            }
            mac.update(p.as_bytes());
        }
        hex::encode(mac.finalize().into_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_plaintext() {
        let b = SecretBox::from_config(None, "mysql://u:p@h/db").unwrap();
        let e = b.encrypt("JBSWY3DPEHPK3PXP");
        assert!(e.starts_with(PREFIX) && !e.contains("JBSWY"));
        assert_ne!(e, b.encrypt("JBSWY3DPEHPK3PXP"), "nonce must differ");
        assert_eq!(b.decrypt(&e).unwrap(), "JBSWY3DPEHPK3PXP");
        assert_eq!(b.decrypt("legacy").unwrap(), "legacy");
        assert_eq!(b.encrypt(""), "");
    }

    #[test]
    fn wrong_key_and_hash() {
        let a = SecretBox::from_config(Some(&"11".repeat(32)), "x").unwrap();
        let b = SecretBox::from_config(Some(&"22".repeat(32)), "x").unwrap();
        assert!(b.decrypt(&a.encrypt("hello")).is_err());
        assert!(SecretBox::from_config(Some("nothex"), "x").is_err());
        assert_eq!(a.keyed_hash(&["a", "b"]), a.keyed_hash(&["a", "b"]));
        assert_ne!(a.keyed_hash(&["a", "b"]), b.keyed_hash(&["a", "b"]));
        assert_ne!(a.keyed_hash(&["ab", ""]), a.keyed_hash(&["a", "b"]));
        assert_eq!(a.keyed_hash(&["x"]).len(), 64);
    }
}
