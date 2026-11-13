use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use hkdf::Hkdf;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use crate::error::{Error, Result};

pub const KEY_LEN: usize = 32;
pub const NONCE_LEN: usize = 24;
pub const TAG_LEN: usize = 16;
pub const SALT_LEN: usize = 32;

#[derive(Clone, Zeroize, ZeroizeOnDrop, PartialEq, Eq)]
pub struct Key([u8; KEY_LEN]);

impl Key {
    pub fn random() -> Result<Self> {
        Ok(Self(random_array()?))
    }
    pub fn from_bytes(bytes: [u8; KEY_LEN]) -> Self {
        Self(bytes)
    }
    pub fn from_slice(bytes: &[u8]) -> Result<Self> {
        let arr: [u8; KEY_LEN] = bytes.try_into().map_err(|_| Error::Corrupt("key length"))?;
        Ok(Self(arr))
    }
    pub fn as_bytes(&self) -> &[u8; KEY_LEN] {
        &self.0
    }
}

impl std::fmt::Debug for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Key(<redacted>)")
    }
}

pub fn fill_random(buf: &mut [u8]) -> Result<()> {
    getrandom::fill(buf).map_err(|_| Error::Rng)
}

pub fn random_array<const N: usize>() -> Result<[u8; N]> {
    let mut out = [0u8; N];
    fill_random(&mut out)?;
    Ok(out)
}

pub fn random_below(bound: u32) -> Result<u32> {
    assert!(bound > 0, "bound must be positive");
    let zone = u32::MAX - (u32::MAX % bound);
    loop {
        let v = u32::from_le_bytes(random_array()?);
        if v < zone {
            return Ok(v % bound);
        }
    }
}

pub fn seal(key: &Key, plaintext: &[u8], aad: &[u8]) -> Result<([u8; NONCE_LEN], Vec<u8>)> {
    let nonce: [u8; NONCE_LEN] = random_array()?;
    let ct = seal_with_nonce(key, &nonce, plaintext, aad)?;
    Ok((nonce, ct))
}

pub fn seal_with_nonce(key: &Key, nonce: &[u8; NONCE_LEN], plaintext: &[u8], aad: &[u8]) -> Result<Vec<u8>> {
    let cipher = XChaCha20Poly1305::new(key.as_bytes().into());
    cipher
        .encrypt(XNonce::from_slice(nonce), Payload { msg: plaintext, aad })
        .map_err(|_| Error::Encrypt)
}

pub fn open(key: &Key, nonce: &[u8], ciphertext: &[u8], aad: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
    if nonce.len() != NONCE_LEN || ciphertext.len() < TAG_LEN {
        return Err(Error::Corrupt("ciphertext framing"));
    }
    let cipher = XChaCha20Poly1305::new(key.as_bytes().into());
    cipher
        .decrypt(XNonce::from_slice(nonce), Payload { msg: ciphertext, aad })
        .map(Zeroizing::new)
        .map_err(|_| Error::InvalidCredentials)
}

pub fn hkdf(ikm: &[u8], salt: &[u8], info: &[u8]) -> Key {
    let hk = Hkdf::<Sha256>::new(Some(salt), ikm);
    let mut out = [0u8; KEY_LEN];
    hk.expand(info, &mut out)
        .expect("32 bytes is a valid HKDF-SHA256 output length");
    Key(out)
}

pub fn sha256(parts: &[&[u8]]) -> [u8; 32] {
    let mut h = Sha256::new();
    for p in parts {
        h.update(p);
    }
    h.finalize().into()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KdfParams {
    pub m_cost_kib: u32,
    pub t_cost: u32,
    pub p_cost: u32,
}

impl KdfParams {
    const MAX_M_KIB: u32 = 4 * 1024 * 1024;
    const MAX_T: u32 = 64;
    const MAX_P: u32 = 16;

    pub const fn strong() -> Self {
        Self {
            m_cost_kib: 256 * 1024,
            t_cost: 3,
            p_cost: 4,
        }
    }
    pub const fn heavy() -> Self {
        Self {
            m_cost_kib: 512 * 1024,
            t_cost: 4,
            p_cost: 4,
        }
    }
    pub const fn moderate() -> Self {
        Self {
            m_cost_kib: 64 * 1024,
            t_cost: 3,
            p_cost: 2,
        }
    }
    pub const fn insecure_test() -> Self {
        Self {
            m_cost_kib: 8 * 1024,
            t_cost: 1,
            p_cost: 1,
        }
    }

    pub fn validate(&self) -> Result<()> {
        let ok = self.m_cost_kib >= 8 * self.p_cost.max(1)
            && self.m_cost_kib <= Self::MAX_M_KIB
            && (1..=Self::MAX_T).contains(&self.t_cost)
            && (1..=Self::MAX_P).contains(&self.p_cost);
        if ok { Ok(()) } else { Err(Error::KdfParams) }
    }
}

pub fn argon2id(secret: &[u8], salt: &[u8], params: &KdfParams) -> Result<Key> {
    params.validate()?;
    let p =
        Params::new(params.m_cost_kib, params.t_cost, params.p_cost, Some(KEY_LEN)).map_err(|_| Error::KdfParams)?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, p);
    let mut out = [0u8; KEY_LEN];
    argon
        .hash_password_into(secret, salt, &mut out)
        .map_err(|_| Error::Kdf)?;
    Ok(Key(out))
}

pub fn calibrate(target_ms: u64) -> KdfParams {
    let mut params = KdfParams::strong();
    let salt = [0u8; SALT_LEN];
    loop {
        let start = std::time::Instant::now();
        if argon2id(b"calibration", &salt, &params).is_err() {
            return KdfParams::strong();
        }
        let elapsed = start.elapsed().as_millis() as u64;
        if elapsed >= target_ms || params.t_cost >= 12 {
            return params;
        }
        let per_pass = (elapsed / params.t_cost as u64).max(1);
        let wanted = (target_ms / per_pass) as u32;
        params.t_cost = wanted.clamp(params.t_cost + 1, 12);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seal_open_roundtrip() {
        let k = Key::random().unwrap();
        let (n, ct) = seal(&k, b"hello", b"aad").unwrap();
        assert_eq!(&*open(&k, &n, &ct, b"aad").unwrap(), b"hello");
    }

    #[test]
    fn open_rejects_wrong_aad_key_and_bitflips() {
        let k = Key::random().unwrap();
        let (n, ct) = seal(&k, b"hello", b"aad").unwrap();
        assert!(open(&k, &n, &ct, b"aaX").is_err());
        assert!(open(&Key::random().unwrap(), &n, &ct, b"aad").is_err());
        for i in 0..ct.len() {
            let mut bad = ct.clone();
            bad[i] ^= 0x01;
            assert!(open(&k, &n, &bad, b"aad").is_err(), "bitflip at {i} accepted");
        }
        assert!(open(&k, &n, &ct[..ct.len() - 1], b"aad").is_err());
    }

    #[test]
    fn argon2id_is_deterministic_and_salted() {
        let p = KdfParams::insecure_test();
        let a = argon2id(b"pw", &[1u8; 32], &p).unwrap();
        let b = argon2id(b"pw", &[1u8; 32], &p).unwrap();
        let c = argon2id(b"pw", &[2u8; 32], &p).unwrap();
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn kdf_bounds_enforced() {
        assert!(
            KdfParams {
                m_cost_kib: u32::MAX,
                t_cost: 1,
                p_cost: 1
            }
            .validate()
            .is_err()
        );
        assert!(
            KdfParams {
                m_cost_kib: 1024,
                t_cost: 0,
                p_cost: 1
            }
            .validate()
            .is_err()
        );
        assert!(KdfParams::strong().validate().is_ok());
    }

    #[test]
    fn random_below_in_range() {
        for _ in 0..1000 {
            assert!(random_below(7).unwrap() < 7);
        }
    }
}
