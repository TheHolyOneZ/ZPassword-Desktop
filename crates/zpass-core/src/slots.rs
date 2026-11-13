use data_encoding::BASE32_NOPAD;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::crypto::{self, KdfParams, Key, NONCE_LEN, SALT_LEN};
use crate::error::{Error, Result};
use crate::util::{b64, now};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YubiKeyFactor {
    pub slot: u8,
    #[serde(with = "b64")]
    pub challenge: Vec<u8>,
    pub serial: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum SlotKind {
    #[serde(rename_all = "camelCase")]
    Password {
        keyfile: bool,
        yubikey: Option<YubiKeyFactor>,
    },
    #[serde(rename_all = "camelCase")]
    RecoveryCode { index: u8 },
    #[serde(rename_all = "camelCase")]
    RecoveryQuestions { questions: Vec<String> },
    #[serde(rename_all = "camelCase")]
    OsKeystore { backend: String, device: String },
}

impl SlotKind {
    pub fn tag(&self) -> &'static str {
        match self {
            SlotKind::Password { .. } => "password",
            SlotKind::RecoveryCode { .. } => "recoveryCode",
            SlotKind::RecoveryQuestions { .. } => "recoveryQuestions",
            SlotKind::OsKeystore { .. } => "osKeystore",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Slot {
    pub id: Uuid,
    pub kind: SlotKind,
    pub kdf: Option<KdfParams>,
    #[serde(with = "b64")]
    pub salt: [u8; SALT_LEN],
    #[serde(with = "b64")]
    pub nonce: [u8; NONCE_LEN],
    #[serde(with = "b64")]
    pub wrapped_key: Vec<u8>,
    pub created_at: i64,
}

fn wrap_aad(vault_id: &Uuid, slot_id: &Uuid, kind: &SlotKind) -> Vec<u8> {
    let mut aad = b"zpass/slot/v1".to_vec();
    aad.extend_from_slice(vault_id.as_bytes());
    aad.extend_from_slice(slot_id.as_bytes());
    aad.extend_from_slice(kind.tag().as_bytes());
    aad
}

impl Slot {
    pub fn wrap(
        vault_id: &Uuid,
        kind: SlotKind,
        kdf: Option<KdfParams>,
        salt: [u8; SALT_LEN],
        kek: &Key,
        vmk: &Key,
    ) -> Result<Self> {
        let id = Uuid::new_v4();
        let (nonce, wrapped_key) = crypto::seal(kek, vmk.as_bytes(), &wrap_aad(vault_id, &id, &kind))?;
        Ok(Self {
            id,
            kind,
            kdf,
            salt,
            nonce,
            wrapped_key,
            created_at: now(),
        })
    }

    pub fn new_salt() -> Result<[u8; SALT_LEN]> {
        crypto::random_array()
    }

    pub fn unwrap(&self, vault_id: &Uuid, kek: &Key) -> Result<Key> {
        let pt = crypto::open(
            kek,
            &self.nonce,
            &self.wrapped_key,
            &wrap_aad(vault_id, &self.id, &self.kind),
        )?;
        Key::from_slice(&pt)
    }
}

#[derive(Default)]
pub struct PasswordFactors<'a> {
    pub keyfile_digest: Option<&'a [u8; 32]>,
    pub yubikey_response: Option<&'a [u8]>,
}

pub fn password_kek(password: &[u8], factors: &PasswordFactors, salt: &[u8], params: &KdfParams) -> Result<Key> {
    let stretched = crypto::argon2id(password, salt, params)?;
    let mut ikm = Zeroizing::new(Vec::with_capacity(32 + 33 + 21));
    ikm.extend_from_slice(stretched.as_bytes());
    if let Some(kf) = factors.keyfile_digest {
        ikm.push(0x01);
        ikm.extend_from_slice(kf);
    }
    if let Some(yk) = factors.yubikey_response {
        ikm.push(0x02);
        ikm.extend_from_slice(yk);
    }
    Ok(crypto::hkdf(&ikm, salt, b"zpass/kek/password/v1"))
}

pub fn keyfile_digest(contents: &[u8]) -> [u8; 32] {
    crypto::sha256(&[b"zpass/keyfile/v1", contents])
}

pub fn recovery_code_kek(code_bytes: &[u8], salt: &[u8]) -> Key {
    crypto::hkdf(code_bytes, salt, b"zpass/kek/recovery-code/v1")
}

pub fn questions_kek(answers: &[String], salt: &[u8], params: &KdfParams) -> Result<Key> {
    let mut joined = Zeroizing::new(String::new());
    for (i, a) in answers.iter().enumerate() {
        if i > 0 {
            joined.push('\u{1f}');
        }
        joined.push_str(&normalize_answer(a));
    }
    let stretched = crypto::argon2id(joined.as_bytes(), salt, params)?;
    Ok(crypto::hkdf(stretched.as_bytes(), salt, b"zpass/kek/questions/v1"))
}

pub fn os_kek(device_secret: &[u8], salt: &[u8]) -> Key {
    crypto::hkdf(device_secret, salt, b"zpass/kek/os/v1")
}

pub fn normalize_answer(a: &str) -> String {
    let mut out = String::with_capacity(a.len());
    let mut pending_space = false;
    for ch in a.chars().flat_map(char::to_lowercase) {
        if ch.is_alphanumeric() {
            if pending_space && !out.is_empty() {
                out.push(' ');
            }
            pending_space = false;
            out.push(ch);
        } else {
            pending_space = true;
        }
    }
    out
}

pub const RECOVERY_CODE_BYTES: usize = 15;
pub const RECOVERY_CODE_COUNT: usize = 10;

pub struct RecoveryCode {
    pub display: Zeroizing<String>,
    pub bytes: Zeroizing<Vec<u8>>,
}

pub fn generate_recovery_code() -> Result<RecoveryCode> {
    let raw: Zeroizing<[u8; RECOVERY_CODE_BYTES]> = Zeroizing::new(crypto::random_array()?);
    let encoded = Zeroizing::new(BASE32_NOPAD.encode(&*raw));
    let mut display = Zeroizing::new(String::with_capacity(29));
    for (i, ch) in encoded.chars().enumerate() {
        if i > 0 && i % 4 == 0 {
            display.push('-');
        }
        display.push(ch);
    }
    Ok(RecoveryCode {
        display,
        bytes: Zeroizing::new(raw.to_vec()),
    })
}

pub fn parse_recovery_code(input: &str) -> Result<Zeroizing<Vec<u8>>> {
    let cleaned: Zeroizing<String> = Zeroizing::new(
        input
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .map(|c| match c.to_ascii_uppercase() {
                '0' => 'O',
                '1' => 'I',
                '8' => 'B',
                o => o,
            })
            .collect(),
    );
    let bytes = BASE32_NOPAD
        .decode(cleaned.as_bytes())
        .map_err(|_| Error::Invalid("That recovery code is not valid.".into()))?;
    if bytes.len() != RECOVERY_CODE_BYTES {
        return Err(Error::Invalid("That recovery code is not valid.".into()));
    }
    Ok(Zeroizing::new(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answer_normalisation() {
        assert_eq!(normalize_answer("  New   York!! "), "new york");
        assert_eq!(normalize_answer("new-york"), "new york");
        assert_eq!(normalize_answer("MÜNCHEN"), "münchen");
    }

    #[test]
    fn recovery_code_roundtrip() {
        let code = generate_recovery_code().unwrap();
        assert_eq!(code.display.len(), 29);
        let parsed = parse_recovery_code(&code.display.to_lowercase()).unwrap();
        assert_eq!(&*parsed, &*code.bytes);
        assert!(parse_recovery_code("AAAA").is_err());
    }

    #[test]
    fn slot_wrap_unwrap_and_binding() {
        let vault = Uuid::new_v4();
        let vmk = Key::random().unwrap();
        let kek = Key::random().unwrap();
        let slot = Slot::wrap(&vault, SlotKind::RecoveryCode { index: 0 }, None, [0; 32], &kek, &vmk).unwrap();
        assert_eq!(slot.unwrap(&vault, &kek).unwrap(), vmk);
        assert!(slot.unwrap(&Uuid::new_v4(), &kek).is_err());
        let mut swapped = slot.clone();
        swapped.kind = SlotKind::OsKeystore {
            backend: "x".into(),
            device: "y".into(),
        };
        assert!(swapped.unwrap(&vault, &kek).is_err());
    }

    #[test]
    fn password_factors_change_kek() {
        let p = KdfParams::insecure_test();
        let salt = [7u8; 32];
        let kf = keyfile_digest(b"file");
        let base = password_kek(b"pw", &PasswordFactors::default(), &salt, &p).unwrap();
        let with_kf = password_kek(
            b"pw",
            &PasswordFactors {
                keyfile_digest: Some(&kf),
                ..Default::default()
            },
            &salt,
            &p,
        )
        .unwrap();
        let with_yk = password_kek(
            b"pw",
            &PasswordFactors {
                yubikey_response: Some(&[1; 20]),
                ..Default::default()
            },
            &salt,
            &p,
        )
        .unwrap();
        assert_ne!(base, with_kf);
        assert_ne!(base, with_yk);
        assert_ne!(with_kf, with_yk);
    }
}
