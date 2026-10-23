//! TOTP (RFC 6238) — bandiuri 에서 옮김.
//! — Google Authenticator·Microsoft Authenticator 등 OTP 앱의 6자리 코드.
//!
//! SHA-1, 30초, 6자리 (OTP 앱들의 기본값). 시계 차이를 위해 앞뒤 한 칸(±30초)까지 인정합니다.

use hmac::{Hmac, Mac};
use rand::RngCore;
use sha1::Sha1;

const STEP: u64 = 30;
const ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

pub fn base32_encode(data: &[u8]) -> String {
    let mut out = String::new();
    let (mut buf, mut bits) = (0u32, 0u32);
    for &b in data {
        buf = (buf << 8) | u32::from(b);
        bits += 8;
        while bits >= 5 {
            out.push(ALPHABET[((buf >> (bits - 5)) & 31) as usize] as char);
            bits -= 5;
        }
    }
    if bits > 0 {
        out.push(ALPHABET[((buf << (5 - bits)) & 31) as usize] as char);
    }
    out
}

pub fn base32_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let (mut buf, mut bits) = (0u32, 0u32);
    for c in s.chars().filter(|c| !c.is_whitespace() && *c != '=') {
        let v = ALPHABET.iter().position(|&a| a as char == c.to_ascii_uppercase())? as u32;
        buf = (buf << 5) | v;
        bits += 5;
        if bits >= 8 {
            out.push(((buf >> (bits - 8)) & 0xff) as u8);
            bits -= 8;
        }
    }
    Some(out)
}

/// 새 비밀값 (20바이트 → base32 32자)
pub fn new_secret() -> String {
    let mut b = [0u8; 20];
    rand::thread_rng().fill_bytes(&mut b);
    base32_encode(&b)
}

/// 시간 칸(time step) 의 6자리 코드
pub fn code_at(secret: &[u8], step: u64) -> String {
    let mut mac = Hmac::<Sha1>::new_from_slice(secret).expect("hmac key");
    mac.update(&step.to_be_bytes());
    let h = mac.finalize().into_bytes();
    let off = (h[19] & 0x0f) as usize;
    let bin = (u32::from(h[off] & 0x7f) << 24)
        | (u32::from(h[off + 1]) << 16)
        | (u32::from(h[off + 2]) << 8)
        | u32::from(h[off + 3]);
    format!("{:06}", bin % 1_000_000)
}

pub fn now_step() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) / STEP
}

/// 코드가 맞으면 맞은 시간 칸을 돌려줍니다 (같은 코드 재사용 막기에 씀)
pub fn verify(secret_b32: &str, code: &str, step: u64) -> Option<u64> {
    let code = code.trim().replace(' ', "");
    if code.len() != 6 || !code.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let key = base32_decode(secret_b32)?;
    [step.saturating_sub(1), step, step + 1].into_iter().find(|s| code_at(&key, *s) == code)
}

/// OTP 앱에 등록할 주소 (QR 코드로 보여 줌)
pub fn otpauth_url(issuer: &str, account: &str, secret_b32: &str) -> String {
    let enc = |s: &str| {
        s.bytes()
            .map(|b| {
                if b.is_ascii_alphanumeric() || b"-._~@".contains(&b) {
                    (b as char).to_string()
                } else {
                    format!("%{b:02X}")
                }
            })
            .collect::<String>()
    };
    format!(
        "otpauth://totp/{}:{}?secret={secret_b32}&issuer={}&algorithm=SHA1&digits=6&period=30",
        enc(issuer),
        enc(account),
        enc(issuer)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc6238_vectors() {
        // RFC 6238 부록 B, SHA-1, 키 "12345678901234567890" (8자리 중 끝 6자리)
        let key = b"12345678901234567890";
        assert_eq!(code_at(key, 59 / 30), "287082");
        assert_eq!(code_at(key, 1111111109 / 30), "081804");
        assert_eq!(code_at(key, 1234567890 / 30), "005924");
        assert_eq!(code_at(key, 2000000000 / 30), "279037");
    }

    #[test]
    fn base32_roundtrip_and_verify_window() {
        assert_eq!(base32_encode(b"12345678901234567890"), "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ");
        let s = new_secret();
        assert_eq!(base32_decode(&s).unwrap().len(), 20);
        let key = base32_decode(&s).unwrap();
        let step = 55_000_000;
        assert_eq!(verify(&s, &code_at(&key, step), step), Some(step));
        assert_eq!(verify(&s, &code_at(&key, step - 1), step), Some(step - 1));
        assert_eq!(verify(&s, &code_at(&key, step - 2), step), None);
        assert_eq!(verify(&s, "12345", step), None);
        assert!(otpauth_url("gageyo", "boss@op.kr", &s).starts_with("otpauth://totp/gageyo:boss@op.kr?secret="));
    }
}
