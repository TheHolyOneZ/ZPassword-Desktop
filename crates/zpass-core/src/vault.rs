use std::path::{Path, PathBuf};

use uuid::Uuid;
use zeroize::Zeroizing;

use crate::crypto::{self, KdfParams, Key};
use crate::error::{Error, Result};
use crate::format::{Container, Header};
use crate::model::{VAULT_SCHEMA, Vault};
use crate::slots::{self, PasswordFactors, Slot, SlotKind, YubiKeyFactor};
use crate::storage;
use crate::util::now;

const PAYLOAD_INFO: &[u8] = b"zpass/payload/v1";

fn writer() -> String {
    format!("zpassword/{}", env!("CARGO_PKG_VERSION"))
}

pub enum Credential<'a> {
    Password {
        password: &'a [u8],
        keyfile_digest: Option<[u8; 32]>,
        yubikey_response: Option<Zeroizing<Vec<u8>>>,
    },
    RecoveryCode(&'a str),
    RecoveryQuestions(&'a [String]),
    OsSecret {
        slot_id: Uuid,
        secret: &'a [u8],
    },
    MasterKey(Key),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnlockedVia {
    Password,
    RecoveryCode { slot_id: Uuid },
    RecoveryQuestions,
    OsKeystore,
    MasterKey,
}

#[derive(Default)]
pub struct NewPasswordFactors {
    pub keyfile_digest: Option<[u8; 32]>,
    pub yubikey: Option<(YubiKeyFactor, Zeroizing<Vec<u8>>)>,
}

#[derive(Debug, Clone)]
pub struct LockedVault {
    pub path: PathBuf,
    pub container: Container,
}

impl LockedVault {
    pub fn load(path: &Path) -> Result<Self> {
        let bytes = std::fs::read(path)?;
        Self::from_bytes(path, &bytes)
    }

    pub fn from_bytes(path: &Path, bytes: &[u8]) -> Result<Self> {
        Ok(Self {
            path: path.to_path_buf(),
            container: Container::parse(bytes)?,
        })
    }

    pub fn header(&self) -> &Header {
        &self.container.header
    }

    pub fn password_slot(&self) -> Option<&Slot> {
        self.header()
            .slots
            .iter()
            .find(|s| matches!(s.kind, SlotKind::Password { .. }))
    }

    pub fn slots_of<'a>(&'a self, tag: &'a str) -> impl Iterator<Item = &'a Slot> + 'a {
        self.header().slots.iter().filter(move |s| s.kind.tag() == tag)
    }

    pub fn unwrap_master_key(&self, cred: &Credential) -> Result<(Key, UnlockedVia)> {
        unwrap_from_header(self.header(), cred)
    }

    pub fn unlock(self, cred: &Credential) -> Result<(OpenVault, UnlockedVia)> {
        let (vmk, via) = self.unwrap_master_key(cred)?;
        let data = decrypt_payload(&self.container, &vmk)?;
        Ok((
            OpenVault {
                path: self.path,
                header: self.container.header,
                vmk,
                data,
            },
            via,
        ))
    }
}

pub fn unwrap_from_header(header: &Header, cred: &Credential) -> Result<(Key, UnlockedVia)> {
    let vault_id = header.vault_id;
    match cred {
        Credential::Password {
            password,
            keyfile_digest,
            yubikey_response,
        } => {
            let slot = header
                .slots
                .iter()
                .find(|s| matches!(s.kind, SlotKind::Password { .. }))
                .ok_or(Error::NotFound("password slot"))?;
            let kdf = slot.kdf.ok_or(Error::Corrupt("password slot without KDF"))?;
            let factors = PasswordFactors {
                keyfile_digest: keyfile_digest.as_ref(),
                yubikey_response: yubikey_response.as_deref().map(|v| v.as_slice()),
            };
            let kek = slots::password_kek(password, &factors, &slot.salt, &kdf)?;
            Ok((slot.unwrap(&vault_id, &kek)?, UnlockedVia::Password))
        }
        Credential::RecoveryCode(code) => {
            let bytes = slots::parse_recovery_code(code)?;
            for slot in header.slots.iter().filter(|s| s.kind.tag() == "recoveryCode") {
                let kek = slots::recovery_code_kek(&bytes, &slot.salt);
                if let Ok(vmk) = slot.unwrap(&vault_id, &kek) {
                    return Ok((vmk, UnlockedVia::RecoveryCode { slot_id: slot.id }));
                }
            }
            Err(Error::InvalidCredentials)
        }
        Credential::RecoveryQuestions(answers) => {
            let slot = header
                .slots
                .iter()
                .find(|s| s.kind.tag() == "recoveryQuestions")
                .ok_or(Error::NotFound("recovery questions"))?;
            let kdf = slot.kdf.ok_or(Error::Corrupt("questions slot without KDF"))?;
            let kek = slots::questions_kek(answers, &slot.salt, &kdf)?;
            Ok((slot.unwrap(&vault_id, &kek)?, UnlockedVia::RecoveryQuestions))
        }
        Credential::OsSecret { slot_id, secret } => {
            let slot = header
                .slots
                .iter()
                .find(|s| &s.id == slot_id)
                .ok_or(Error::NotFound("device slot"))?;
            let kek = slots::os_kek(secret, &slot.salt);
            Ok((slot.unwrap(&vault_id, &kek)?, UnlockedVia::OsKeystore))
        }
        Credential::MasterKey(k) => Ok((k.clone(), UnlockedVia::MasterKey)),
    }
}

fn payload_key(vmk: &Key, vault_id: &Uuid) -> Key {
    crypto::hkdf(vmk.as_bytes(), vault_id.as_bytes(), PAYLOAD_INFO)
}

fn decrypt_payload(c: &Container, vmk: &Key) -> Result<Vault> {
    let key = payload_key(vmk, &c.header.vault_id);
    let plain = crypto::open(&key, &c.nonce, &c.ciphertext, &c.aad)
        .map_err(|_| Error::Corrupt("payload authentication failed"))?;
    let vault: Vault = serde_json::from_slice(&plain).map_err(|_| Error::Corrupt("payload"))?;
    if vault.schema > VAULT_SCHEMA {
        return Err(Error::Invalid(
            "This vault was created by a newer version of ZPassword.".into(),
        ));
    }
    Ok(vault)
}

pub struct OpenVault {
    pub path: PathBuf,
    pub header: Header,
    vmk: Key,
    pub data: Vault,
}

impl OpenVault {
    pub fn create(
        path: &Path,
        name: &str,
        password: &[u8],
        factors: NewPasswordFactors,
        kdf: KdfParams,
    ) -> Result<Self> {
        let vmk = Key::random()?;
        let header = Header {
            vault_id: Uuid::new_v4(),
            created_at: now(),
            writer: writer(),
            slots: vec![],
        };
        let mut v = Self {
            path: path.to_path_buf(),
            header,
            vmk,
            data: Vault::new(name),
        };
        v.set_password(password, factors, kdf)?;
        Ok(v)
    }

    pub fn vault_id(&self) -> Uuid {
        self.header.vault_id
    }

    pub fn master_key(&self) -> &Key {
        &self.vmk
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut header = self.header.clone();
        header.writer = writer();
        let aad = Container::build_aad(&header)?;
        let mut plain = Zeroizing::new(Vec::with_capacity(4096));
        serde_json::to_writer(&mut *plain, &self.data)?;
        let (nonce, ct) = crypto::seal(&payload_key(&self.vmk, &header.vault_id), &plain, &aad)?;
        Ok(Container::assemble(&aad, &nonce, &ct))
    }

    pub fn save(&self) -> Result<()> {
        self.save_opts(true)
    }

    pub fn save_opts(&self, backup: bool) -> Result<()> {
        let bytes = self.to_bytes()?;
        let check = Container::parse(&bytes)?;
        decrypt_payload(&check, &self.vmk)?;
        if backup {
            storage::rotate_backup(&self.path, self.data.settings.backups_keep as usize)?;
        }
        storage::atomic_write(&self.path, &bytes)
    }

    pub fn verify(&self, cred: &Credential) -> Result<UnlockedVia> {
        let (vmk, via) = unwrap_from_header(&self.header, cred)?;
        if vmk != self.vmk {
            return Err(Error::InvalidCredentials);
        }
        Ok(via)
    }

    fn push_slot(&mut self, kind: SlotKind, kdf: Option<KdfParams>, salt: [u8; 32], kek: &Key) -> Result<Uuid> {
        let slot = Slot::wrap(&self.header.vault_id, kind, kdf, salt, kek, &self.vmk)?;
        let id = slot.id;
        self.header.slots.push(slot);
        Ok(id)
    }

    pub fn remove_slot(&mut self, id: &Uuid) -> Result<()> {
        let idx = self
            .header
            .slots
            .iter()
            .position(|s| &s.id == id)
            .ok_or(Error::NotFound("slot"))?;
        if matches!(self.header.slots[idx].kind, SlotKind::Password { .. }) {
            return Err(Error::Invalid("The master password slot cannot be removed.".into()));
        }
        self.header.slots.remove(idx);
        Ok(())
    }

    fn remove_slots_where(&mut self, f: impl Fn(&SlotKind) -> bool) {
        self.header.slots.retain(|s| !f(&s.kind));
    }

    pub fn set_password(&mut self, password: &[u8], factors: NewPasswordFactors, kdf: KdfParams) -> Result<()> {
        if password.is_empty() {
            return Err(Error::Invalid("The master password cannot be empty.".into()));
        }
        let salt = Slot::new_salt()?;
        let yk_factor = factors.yubikey.as_ref().map(|(f, _)| f.clone());
        let kek = slots::password_kek(
            password,
            &PasswordFactors {
                keyfile_digest: factors.keyfile_digest.as_ref(),
                yubikey_response: factors.yubikey.as_ref().map(|(_, r)| r.as_slice()),
            },
            &salt,
            &kdf,
        )?;
        self.remove_slots_where(|k| matches!(k, SlotKind::Password { .. }));
        self.push_slot(
            SlotKind::Password {
                keyfile: factors.keyfile_digest.is_some(),
                yubikey: yk_factor,
            },
            Some(kdf),
            salt,
            &kek,
        )?;
        Ok(())
    }

    pub fn regenerate_recovery_codes(&mut self) -> Result<Vec<Zeroizing<String>>> {
        self.remove_slots_where(|k| matches!(k, SlotKind::RecoveryCode { .. }));
        let mut out = Vec::with_capacity(slots::RECOVERY_CODE_COUNT);
        for i in 0..slots::RECOVERY_CODE_COUNT {
            let code = slots::generate_recovery_code()?;
            let salt = Slot::new_salt()?;
            let kek = slots::recovery_code_kek(&code.bytes, &salt);
            self.push_slot(SlotKind::RecoveryCode { index: i as u8 }, None, salt, &kek)?;
            out.push(code.display);
        }
        Ok(out)
    }

    pub fn clear_recovery_codes(&mut self) {
        self.remove_slots_where(|k| matches!(k, SlotKind::RecoveryCode { .. }));
    }

    pub fn set_recovery_questions(&mut self, questions: Vec<String>, answers: &[String], kdf: KdfParams) -> Result<()> {
        if questions.len() < 2 || questions.len() != answers.len() {
            return Err(Error::Invalid(
                "Provide at least two questions, each with an answer.".into(),
            ));
        }
        if answers.iter().any(|a| slots::normalize_answer(a).chars().count() < 3) {
            return Err(Error::Invalid(
                "Each answer needs at least three letters or digits.".into(),
            ));
        }
        let salt = Slot::new_salt()?;
        let kek = slots::questions_kek(answers, &salt, &kdf)?;
        self.remove_slots_where(|k| matches!(k, SlotKind::RecoveryQuestions { .. }));
        self.push_slot(SlotKind::RecoveryQuestions { questions }, Some(kdf), salt, &kek)?;
        Ok(())
    }

    pub fn clear_recovery_questions(&mut self) {
        self.remove_slots_where(|k| matches!(k, SlotKind::RecoveryQuestions { .. }));
    }

    pub fn add_os_slot(&mut self, backend: &str, device: &str, device_secret: &[u8]) -> Result<Uuid> {
        let salt = Slot::new_salt()?;
        let kek = slots::os_kek(device_secret, &salt);
        self.push_slot(
            SlotKind::OsKeystore {
                backend: backend.into(),
                device: device.into(),
            },
            None,
            salt,
            &kek,
        )
    }

    pub fn export_encrypted(&self, password: &[u8], kdf: KdfParams) -> Result<Vec<u8>> {
        let mut copy = OpenVault::create(
            &self.path,
            &self.data.name,
            password,
            NewPasswordFactors::default(),
            kdf,
        )?;
        copy.data = self.data.clone();
        copy.data.security = Default::default();
        copy.to_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Field, FieldKind, Item};

    fn pw(p: &str) -> Credential<'_> {
        Credential::Password {
            password: p.as_bytes(),
            keyfile_digest: None,
            yubikey_response: None,
        }
    }

    fn sample(dir: &Path) -> OpenVault {
        let mut v = OpenVault::create(
            &dir.join("t.zpv"),
            "Test",
            b"correct horse",
            Default::default(),
            KdfParams::insecure_test(),
        )
        .unwrap();
        let serial = v.data.take_serial();
        v.data.items.push(Item {
            id: Uuid::new_v4(),
            serial,
            template: "login".into(),
            title: "GitHub".into(),
            description: String::new(),
            folder_id: None,
            labels: vec![],
            favorite: false,
            fields: vec![Field::new("Password", FieldKind::Password, "hunter2")],
            history: vec![],
            created_at: now(),
            updated_at: now(),
            trashed_at: None,
        });
        v.save().unwrap();
        v
    }

    #[test]
    fn create_save_unlock_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let v = sample(dir.path());
        let locked = LockedVault::load(&v.path).unwrap();
        let (open, via) = locked.unlock(&pw("correct horse")).unwrap();
        assert_eq!(via, UnlockedVia::Password);
        assert_eq!(open.data.items[0].title, "GitHub");
        assert_eq!(open.data.items[0].fields[0].value, "hunter2");
        assert!(LockedVault::load(&v.path).unwrap().unlock(&pw("wrong")).is_err());
    }

    #[test]
    fn payload_tamper_and_truncation_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let v = sample(dir.path());
        let bytes = std::fs::read(&v.path).unwrap();
        let mut bad = bytes.clone();
        let last = bad.len() - 5;
        bad[last] ^= 0x80;
        assert!(
            LockedVault::from_bytes(&v.path, &bad)
                .unwrap()
                .unlock(&pw("correct horse"))
                .is_err()
        );
        assert!(
            LockedVault::from_bytes(&v.path, &bytes[..bytes.len() - 20])
                .and_then(|l| l.unlock(&pw("correct horse")).map(|_| ()))
                .is_err()
        );
        let s = String::from_utf8_lossy(&bytes).replace("zpassword/", "zpassworX/");
        let l = LockedVault::from_bytes(&v.path, s.as_bytes());
        assert!(l.and_then(|l| l.unlock(&pw("correct horse")).map(|_| ())).is_err());
    }

    #[test]
    fn keyfile_and_yubikey_factors_required() {
        let dir = tempfile::tempdir().unwrap();
        let mut v = sample(dir.path());
        let kf = slots::keyfile_digest(b"my keyfile");
        let yk = YubiKeyFactor {
            slot: 2,
            challenge: vec![9; 32],
            serial: None,
        };
        let resp = Zeroizing::new(vec![5u8; 20]);
        v.set_password(
            b"pw2",
            NewPasswordFactors {
                keyfile_digest: Some(kf),
                yubikey: Some((yk, resp.clone())),
            },
            KdfParams::insecure_test(),
        )
        .unwrap();
        v.save().unwrap();
        let l = LockedVault::load(&v.path).unwrap();
        assert!(l.unwrap_master_key(&pw("pw2")).is_err());
        assert!(
            l.unwrap_master_key(&Credential::Password {
                password: b"pw2",
                keyfile_digest: Some(kf),
                yubikey_response: None
            })
            .is_err()
        );
        let ok = Credential::Password {
            password: b"pw2",
            keyfile_digest: Some(kf),
            yubikey_response: Some(resp),
        };
        assert!(l.unlock(&ok).is_ok());
    }

    #[test]
    fn recovery_codes_unlock_and_are_single_use() {
        let dir = tempfile::tempdir().unwrap();
        let mut v = sample(dir.path());
        let codes = v.regenerate_recovery_codes().unwrap();
        assert_eq!(codes.len(), 10);
        v.save().unwrap();

        let (mut open, via) = LockedVault::load(&v.path)
            .unwrap()
            .unlock(&Credential::RecoveryCode(&codes[3]))
            .unwrap();
        let UnlockedVia::RecoveryCode { slot_id } = via else {
            panic!("wrong via")
        };
        open.remove_slot(&slot_id).unwrap();
        open.save().unwrap();

        assert!(
            LockedVault::load(&v.path)
                .unwrap()
                .unlock(&Credential::RecoveryCode(&codes[3]))
                .is_err()
        );
        assert!(
            LockedVault::load(&v.path)
                .unwrap()
                .unlock(&Credential::RecoveryCode(&codes[4]))
                .is_ok()
        );
    }

    #[test]
    fn recovery_questions_unlock_normalised() {
        let dir = tempfile::tempdir().unwrap();
        let mut v = sample(dir.path());
        let q = vec!["First pet?".to_string(), "Birth city?".to_string()];
        v.set_recovery_questions(
            q,
            &["Rex the Dog".into(), "New York".into()],
            KdfParams::insecure_test(),
        )
        .unwrap();
        v.save().unwrap();
        let answers = vec!["  rex THE dog ".to_string(), "new-york".to_string()];
        assert!(
            LockedVault::load(&v.path)
                .unwrap()
                .unlock(&Credential::RecoveryQuestions(&answers))
                .is_ok()
        );
        let wrong = vec!["rex".to_string(), "new york".to_string()];
        assert!(
            LockedVault::load(&v.path)
                .unwrap()
                .unlock(&Credential::RecoveryQuestions(&wrong))
                .is_err()
        );
    }

    #[test]
    fn os_slot_unlock_and_removal() {
        let dir = tempfile::tempdir().unwrap();
        let mut v = sample(dir.path());
        let id = v.add_os_slot("test", "host", &[3u8; 32]).unwrap();
        v.save().unwrap();
        let l = LockedVault::load(&v.path).unwrap();
        assert!(
            l.unwrap_master_key(&Credential::OsSecret {
                slot_id: id,
                secret: &[3u8; 32]
            })
            .is_ok()
        );
        assert!(
            l.unwrap_master_key(&Credential::OsSecret {
                slot_id: id,
                secret: &[4u8; 32]
            })
            .is_err()
        );
        v.remove_slot(&id).unwrap();
        v.save().unwrap();
        assert!(
            LockedVault::load(&v.path)
                .unwrap()
                .unwrap_master_key(&Credential::OsSecret {
                    slot_id: id,
                    secret: &[3u8; 32]
                })
                .is_err()
        );
    }

    #[test]
    fn password_change_keeps_other_slots() {
        let dir = tempfile::tempdir().unwrap();
        let mut v = sample(dir.path());
        let codes = v.regenerate_recovery_codes().unwrap();
        v.set_password(b"new password", Default::default(), KdfParams::insecure_test())
            .unwrap();
        v.save().unwrap();
        assert!(
            LockedVault::load(&v.path)
                .unwrap()
                .unlock(&pw("correct horse"))
                .is_err()
        );
        assert!(LockedVault::load(&v.path).unwrap().unlock(&pw("new password")).is_ok());
        assert!(
            LockedVault::load(&v.path)
                .unwrap()
                .unlock(&Credential::RecoveryCode(&codes[0]))
                .is_ok()
        );
        assert!(
            v.remove_slot(&LockedVault::load(&v.path).unwrap().password_slot().unwrap().id)
                .is_err()
        );
    }

    #[test]
    fn encrypted_export_is_independent() {
        let dir = tempfile::tempdir().unwrap();
        let v = sample(dir.path());
        let bytes = v.export_encrypted(b"backup pw", KdfParams::insecure_test()).unwrap();
        let l = LockedVault::from_bytes(Path::new("b.zpv"), &bytes).unwrap();
        assert_ne!(l.header().vault_id, v.vault_id());
        let (open, _) = l.unlock(&pw("backup pw")).unwrap();
        assert_eq!(open.data.items.len(), 1);
    }

    #[test]
    fn backups_are_written_and_openable() {
        let dir = tempfile::tempdir().unwrap();
        let mut v = sample(dir.path());
        v.data.name = "Renamed".into();
        v.save().unwrap();
        let backups = storage::list_backups(&v.path).unwrap();
        assert!(!backups.is_empty());
        let (old, _) = LockedVault::load(&backups[0].path)
            .unwrap()
            .unlock(&pw("correct horse"))
            .unwrap();
        assert_eq!(old.data.name, "Test");
    }
}
