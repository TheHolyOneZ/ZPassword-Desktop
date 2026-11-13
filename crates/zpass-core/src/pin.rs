use zeroize::Zeroizing;

use crate::crypto::{self, KdfParams, Key, NONCE_LEN, SALT_LEN};
use crate::error::{Error, Result};

pub const MAX_ATTEMPTS: u8 = 5;

pub struct PinWrap {
    ephemeral: Key,
    salt: [u8; SALT_LEN],
    nonce: [u8; NONCE_LEN],
    wrapped: Vec<u8>,
    attempts_left: u8,
    kdf: KdfParams,
}

impl std::fmt::Debug for PinWrap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PinWrap")
            .field("attempts_left", &self.attempts_left)
            .finish_non_exhaustive()
    }
}

fn kek(pin: &str, ephemeral: &Key, salt: &[u8], kdf: &KdfParams) -> Result<Key> {
    let stretched = crypto::argon2id(pin.as_bytes(), salt, kdf)?;
    let mut ikm = Zeroizing::new([0u8; 64]);
    ikm[..32].copy_from_slice(stretched.as_bytes());
    ikm[32..].copy_from_slice(ephemeral.as_bytes());
    Ok(crypto::hkdf(&*ikm, salt, b"zpass/kek/pin/v1"))
}

pub fn validate_pin(pin: &str) -> Result<()> {
    let n = pin.chars().count();
    if !(4..=12).contains(&n) || !pin.chars().all(|c| c.is_ascii_digit()) {
        return Err(Error::Invalid("The PIN must be 4–12 digits.".into()));
    }
    Ok(())
}

impl PinWrap {
    pub fn new(pin: &str, vmk: &Key) -> Result<Self> {
        Self::with_kdf(pin, vmk, KdfParams::moderate())
    }

    pub fn with_kdf(pin: &str, vmk: &Key, kdf: KdfParams) -> Result<Self> {
        validate_pin(pin)?;
        let ephemeral = Key::random()?;
        let salt = crypto::random_array()?;
        let k = kek(pin, &ephemeral, &salt, &kdf)?;
        let (nonce, wrapped) = crypto::seal(&k, vmk.as_bytes(), b"zpass/pin")?;
        Ok(Self {
            ephemeral,
            salt,
            nonce,
            wrapped,
            attempts_left: MAX_ATTEMPTS,
            kdf,
        })
    }

    pub fn attempts_left(&self) -> u8 {
        self.attempts_left
    }

    pub fn is_exhausted(&self) -> bool {
        self.attempts_left == 0
    }

    pub fn try_unwrap(&mut self, pin: &str) -> Result<Key> {
        if self.is_exhausted() {
            return Err(Error::Invalid(
                "Too many wrong PINs. Unlock with your master password.".into(),
            ));
        }
        let k = kek(pin, &self.ephemeral, &self.salt, &self.kdf)?;
        match crypto::open(&k, &self.nonce, &self.wrapped, b"zpass/pin") {
            Ok(pt) => {
                self.attempts_left = MAX_ATTEMPTS;
                Key::from_slice(&pt)
            }
            Err(_) => {
                self.attempts_left -= 1;
                if self.attempts_left == 0 {
                    self.wrapped.iter_mut().for_each(|b| *b = 0);
                    self.wrapped.clear();
                }
                Err(Error::InvalidCredentials)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pin_unwraps_and_locks_out() {
        let vmk = Key::random().unwrap();
        let mut w = PinWrap::with_kdf("1234", &vmk, KdfParams::insecure_test()).unwrap();
        assert_eq!(w.try_unwrap("1234").unwrap(), vmk);
        for _ in 0..MAX_ATTEMPTS {
            assert!(w.try_unwrap("0000").is_err());
        }
        assert!(w.is_exhausted());
        assert!(w.try_unwrap("1234").is_err());
    }

    #[test]
    fn pin_validation() {
        assert!(validate_pin("123").is_err());
        assert!(validate_pin("12a4").is_err());
        assert!(validate_pin("123456").is_ok());
    }
}
