use data_encoding::BASE32_NOPAD;
use hmac::{Hmac, Mac};
use subtle::ConstantTimeEq;

use crate::error::{Error, Result};
use crate::model::{TotpAlgorithm, TotpConfig};

fn hotp(cfg: &TotpConfig, counter: u64) -> u32 {
    let msg = counter.to_be_bytes();
    let digest: Vec<u8> = match cfg.algorithm {
        TotpAlgorithm::Sha1 => {
            let mut m = Hmac::<sha1::Sha1>::new_from_slice(&cfg.secret).expect("hmac accepts any key length");
            m.update(&msg);
            m.finalize().into_bytes().to_vec()
        }
        TotpAlgorithm::Sha256 => {
            let mut m = Hmac::<sha2::Sha256>::new_from_slice(&cfg.secret).expect("hmac accepts any key length");
            m.update(&msg);
            m.finalize().into_bytes().to_vec()
        }
        TotpAlgorithm::Sha512 => {
            let mut m = Hmac::<sha2::Sha512>::new_from_slice(&cfg.secret).expect("hmac accepts any key length");
            m.update(&msg);
            m.finalize().into_bytes().to_vec()
        }
    };
    let offset = (digest[digest.len() - 1] & 0x0f) as usize;
    let bin = u32::from_be_bytes([
        digest[offset] & 0x7f,
        digest[offset + 1],
        digest[offset + 2],
        digest[offset + 3],
    ]);
    bin % 10u32.pow(cfg.digits)
}

pub fn step(cfg: &TotpConfig, unix: u64) -> u64 {
    unix / cfg.period.max(1)
}

pub fn code_at(cfg: &TotpConfig, unix: u64) -> String {
    format!("{:0width$}", hotp(cfg, step(cfg, unix)), width = cfg.digits as usize)
}

pub fn remaining(cfg: &TotpConfig, unix: u64) -> u64 {
    cfg.period - unix % cfg.period.max(1)
}

pub fn verify(cfg: &TotpConfig, code: &str, unix: u64) -> Option<u64> {
    let code: String = code.chars().filter(|c| !c.is_whitespace()).collect();
    if code.len() != cfg.digits as usize {
        return None;
    }
    let now = step(cfg, unix);
    let mut accepted = None;
    for s in [now.saturating_sub(1), now, now + 1] {
        let expected = format!("{:0width$}", hotp(cfg, s), width = cfg.digits as usize);
        if bool::from(expected.as_bytes().ct_eq(code.as_bytes())) && s > cfg.last_step {
            accepted = Some(s);
        }
    }
    accepted
}

pub fn new_secret() -> Result<TotpConfig> {
    Ok(TotpConfig {
        secret: crate::crypto::random_array::<20>()?.to_vec(),
        algorithm: TotpAlgorithm::Sha1,
        digits: 6,
        period: 30,
        last_step: 0,
    })
}

pub fn secret_base32(cfg: &TotpConfig) -> String {
    BASE32_NOPAD.encode(&cfg.secret)
}

fn alg_name(a: &TotpAlgorithm) -> &'static str {
    match a {
        TotpAlgorithm::Sha1 => "SHA1",
        TotpAlgorithm::Sha256 => "SHA256",
        TotpAlgorithm::Sha512 => "SHA512",
    }
}

fn pct(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn unpct(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Some(v) = std::str::from_utf8(&bytes[i + 1..i + 3])
                .ok()
                .and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(if bytes[i] == b'+' { b' ' } else { bytes[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn to_uri(cfg: &TotpConfig, issuer: &str, account: &str) -> String {
    format!(
        "otpauth://totp/{}:{}?secret={}&issuer={}&algorithm={}&digits={}&period={}",
        pct(issuer),
        pct(account),
        secret_base32(cfg),
        pct(issuer),
        alg_name(&cfg.algorithm),
        cfg.digits,
        cfg.period
    )
}

fn decode_secret(s: &str) -> Result<Vec<u8>> {
    let cleaned: String = s
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-' && *c != '=')
        .collect::<String>()
        .to_uppercase();
    BASE32_NOPAD
        .decode(cleaned.as_bytes())
        .map_err(|_| Error::Invalid("The one-time code secret is not valid base32.".into()))
}

pub fn parse(input: &str) -> Result<TotpConfig> {
    let input = input.trim();
    let mut cfg = TotpConfig {
        secret: vec![],
        algorithm: TotpAlgorithm::Sha1,
        digits: 6,
        period: 30,
        last_step: 0,
    };
    if let Some(rest) = input.strip_prefix("otpauth://") {
        let (kind, rest) = rest
            .split_once('/')
            .ok_or_else(|| Error::Invalid("Malformed otpauth URI.".into()))?;
        if !kind.eq_ignore_ascii_case("totp") {
            return Err(Error::Invalid("Only time-based (TOTP) codes are supported.".into()));
        }
        let query = rest.split_once('?').map(|(_, q)| q).unwrap_or("");
        for pair in query.split('&') {
            let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
            let v = unpct(v);
            match k.to_ascii_lowercase().as_str() {
                "secret" => cfg.secret = decode_secret(&v)?,
                "algorithm" => {
                    cfg.algorithm = match v.to_ascii_uppercase().as_str() {
                        "SHA256" => TotpAlgorithm::Sha256,
                        "SHA512" => TotpAlgorithm::Sha512,
                        _ => TotpAlgorithm::Sha1,
                    }
                }
                "digits" => cfg.digits = v.parse().unwrap_or(6),
                "period" => cfg.period = v.parse().unwrap_or(30),
                _ => {}
            }
        }
    } else {
        cfg.secret = decode_secret(input)?;
    }
    if cfg.secret.is_empty() || !(6..=8).contains(&cfg.digits) || !(1..=300).contains(&cfg.period) {
        return Err(Error::Invalid("Unsupported one-time code settings.".into()));
    }
    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(alg: TotpAlgorithm, secret: &[u8]) -> TotpConfig {
        TotpConfig {
            secret: secret.to_vec(),
            algorithm: alg,
            digits: 8,
            period: 30,
            last_step: 0,
        }
    }

    #[test]
    fn rfc6238_vectors() {
        let s1 = b"12345678901234567890";
        let s256 = b"12345678901234567890123456789012";
        let s512 = b"1234567890123456789012345678901234567890123456789012345678901234";
        let cases: [(u64, &str, &str, &str); 3] = [
            (59, "94287082", "46119246", "90693936"),
            (1111111109, "07081804", "68084774", "25091201"),
            (20000000000, "65353130", "77737706", "47863826"),
        ];
        for (t, a, b, c) in cases {
            assert_eq!(code_at(&cfg(TotpAlgorithm::Sha1, s1), t), a);
            assert_eq!(code_at(&cfg(TotpAlgorithm::Sha256, s256), t), b);
            assert_eq!(code_at(&cfg(TotpAlgorithm::Sha512, s512), t), c);
        }
    }

    #[test]
    fn verify_window_and_replay() {
        let mut c = cfg(TotpAlgorithm::Sha1, b"12345678901234567890");
        let code = code_at(&c, 1111111109);
        let s = verify(&c, &code, 1111111109).unwrap();
        assert!(verify(&c, &code, 1111111109 + 30).is_some());
        assert!(verify(&c, &code, 1111111109 + 90).is_none());
        c.last_step = s;
        assert!(verify(&c, &code, 1111111109).is_none(), "replay accepted");
    }

    #[test]
    fn uri_roundtrip() {
        let c = new_secret().unwrap();
        let uri = to_uri(&c, "ZPassword", "My Vault");
        let p = parse(&uri).unwrap();
        assert_eq!(p.secret, c.secret);
        assert_eq!(p.digits, 6);
        assert!(parse("JBSWY3DPEHPK3PXP").is_ok());
        assert!(parse("not base32 !!").is_err());
        assert!(parse("otpauth://hotp/x?secret=JBSWY3DPEHPK3PXP").is_err());
    }
}
