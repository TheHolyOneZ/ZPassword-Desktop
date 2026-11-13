use uuid::Uuid;
use zeroize::Zeroizing;

use crate::error::{ApiError, ApiResult};

pub fn backend() -> Option<&'static str> {
    if cfg!(windows) {
        Some("windowsHello")
    } else if cfg!(target_os = "linux") {
        Some("secretService")
    } else {
        None
    }
}

pub struct Enrollment {
    pub secret: Zeroizing<Vec<u8>>,
    pub challenge: Option<String>,
}

#[cfg(target_os = "linux")]
mod imp {
    use super::*;

    fn entry(vault_id: &Uuid, slot_id: &Uuid) -> ApiResult<keyring::Entry> {
        keyring::Entry::new("ZPassword", &format!("vault:{vault_id}:{slot_id}"))
            .map_err(|e| ApiError::new("osUnlock", format!("Secret Service unavailable: {e}")))
    }

    pub fn available() -> bool {
        keyring::Entry::new("ZPassword", "probe").is_ok()
    }

    pub fn enroll(_vault_id: &Uuid) -> ApiResult<Enrollment> {
        let secret: [u8; 32] = zpass_core::crypto::random_array()?;
        Ok(Enrollment {
            secret: Zeroizing::new(secret.to_vec()),
            challenge: None,
        })
    }

    pub fn store(vault_id: &Uuid, slot_id: &Uuid, e: &Enrollment) -> ApiResult<()> {
        entry(vault_id, slot_id)?
            .set_secret(&e.secret)
            .map_err(|e| ApiError::new("osUnlock", format!("Could not store the device key: {e}")))
    }

    pub fn retrieve(vault_id: &Uuid, slot_id: &Uuid, _challenge: Option<&str>) -> ApiResult<Zeroizing<Vec<u8>>> {
        entry(vault_id, slot_id)?
            .get_secret()
            .map(Zeroizing::new)
            .map_err(|e| ApiError::new("osUnlock", format!("Device key unavailable: {e}")))
    }

    pub fn forget(vault_id: &Uuid, slot_id: &Uuid) {
        if let Ok(e) = entry(vault_id, slot_id) {
            let _ = e.delete_credential();
        }
    }
}

#[cfg(windows)]
mod imp {
    use super::*;
    use data_encoding::BASE64;
    use windows::Security::Credentials::{KeyCredentialCreationOption, KeyCredentialManager, KeyCredentialStatus};
    use windows::Security::Cryptography::CryptographicBuffer;
    use windows::core::{Array, HSTRING};

    const KEY_NAME: &str = "ZPassword Vault Unlock";

    fn hello_err(msg: &str) -> ApiError {
        ApiError::new("osUnlock", format!("Windows Hello: {msg}"))
    }

    pub fn available() -> bool {
        KeyCredentialManager::IsSupportedAsync()
            .and_then(|op| op.get())
            .unwrap_or(false)
    }

    fn credential(create: bool) -> ApiResult<windows::Security::Credentials::KeyCredential> {
        let name = HSTRING::from(KEY_NAME);
        let res = KeyCredentialManager::OpenAsync(&name)
            .and_then(|op| op.get())
            .map_err(|e| hello_err(&e.message()))?;
        let res = match res.Status().map_err(|e| hello_err(&e.message()))? {
            KeyCredentialStatus::Success => res,
            KeyCredentialStatus::NotFound if create => {
                KeyCredentialManager::RequestCreateAsync(&name, KeyCredentialCreationOption::FailIfExists)
                    .and_then(|op| op.get())
                    .map_err(|e| hello_err(&e.message()))?
            }
            KeyCredentialStatus::UserCanceled => return Err(hello_err("cancelled")),
            s => return Err(hello_err(&format!("unavailable ({s:?})"))),
        };
        if res.Status().map_err(|e| hello_err(&e.message()))? != KeyCredentialStatus::Success {
            return Err(hello_err("verification failed or was cancelled"));
        }
        res.Credential().map_err(|e| hello_err(&e.message()))
    }

    fn sign(challenge: &[u8], create: bool) -> ApiResult<Zeroizing<Vec<u8>>> {
        let cred = credential(create)?;
        let buf = CryptographicBuffer::CreateFromByteArray(challenge).map_err(|e| hello_err(&e.message()))?;
        let sig = cred
            .RequestSignAsync(&buf)
            .and_then(|op| op.get())
            .map_err(|e| hello_err(&e.message()))?;
        if sig.Status().map_err(|e| hello_err(&e.message()))? != KeyCredentialStatus::Success {
            return Err(hello_err("verification failed or was cancelled"));
        }
        let mut out = Array::<u8>::new();
        CryptographicBuffer::CopyToByteArray(&sig.Result().map_err(|e| hello_err(&e.message()))?, &mut out)
            .map_err(|e| hello_err(&e.message()))?;
        Ok(Zeroizing::new(
            zpass_core::crypto::sha256(&[b"zpass/hello/v1", out.as_slice()]).to_vec(),
        ))
    }

    pub fn enroll(_vault_id: &Uuid) -> ApiResult<Enrollment> {
        let challenge: [u8; 32] = zpass_core::crypto::random_array()?;
        let secret = sign(&challenge, true)?;
        Ok(Enrollment {
            secret,
            challenge: Some(BASE64.encode(&challenge)),
        })
    }

    pub fn store(_vault_id: &Uuid, _slot_id: &Uuid, _e: &Enrollment) -> ApiResult<()> {
        Ok(())
    }

    pub fn retrieve(_vault_id: &Uuid, _slot_id: &Uuid, challenge: Option<&str>) -> ApiResult<Zeroizing<Vec<u8>>> {
        let c = challenge.ok_or_else(|| hello_err("missing challenge"))?;
        let bytes = BASE64
            .decode(c.as_bytes())
            .map_err(|_| hello_err("corrupt challenge"))?;
        sign(&bytes, false)
    }

    pub fn forget(_vault_id: &Uuid, _slot_id: &Uuid) {}
}

#[cfg(not(any(windows, target_os = "linux")))]
mod imp {
    use super::*;
    pub fn available() -> bool {
        false
    }
    pub fn enroll(_: &Uuid) -> ApiResult<Enrollment> {
        Err(ApiError::new("osUnlock", "Not supported on this platform"))
    }
    pub fn store(_: &Uuid, _: &Uuid, _: &Enrollment) -> ApiResult<()> {
        Ok(())
    }
    pub fn retrieve(_: &Uuid, _: &Uuid, _: Option<&str>) -> ApiResult<Zeroizing<Vec<u8>>> {
        Err(ApiError::new("osUnlock", "Not supported on this platform"))
    }
    pub fn forget(_: &Uuid, _: &Uuid) {}
}

pub use imp::{available, enroll, forget, retrieve, store};
