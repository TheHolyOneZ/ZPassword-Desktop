pub mod items;
pub mod security;
pub mod session;
pub mod tools;
pub mod transfer;

use std::path::Path;

use serde::Deserialize;
use zeroize::Zeroizing;
use zpass_core::slots::{YubiKeyFactor, keyfile_digest};
use zpass_core::vault::NewPasswordFactors;

use crate::error::{ApiError, ApiResult};
use crate::platform::yubikey;

const MAX_KEYFILE: u64 = 64 * 1024 * 1024;

pub fn read_keyfile(path: &Path) -> ApiResult<[u8; 32]> {
    let meta = std::fs::metadata(path).map_err(|_| ApiError::invalid("The keyfile could not be read."))?;
    if meta.len() == 0 || meta.len() > MAX_KEYFILE {
        return Err(ApiError::invalid("Keyfiles must be between 1 byte and 64 MiB."));
    }
    let bytes = Zeroizing::new(std::fs::read(path)?);
    Ok(keyfile_digest(&bytes))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YubiKeyChoice {
    pub slot: u8,
    pub serial: Option<u32>,
}

pub fn new_factors(keyfile: Option<&str>, yubikey_choice: Option<YubiKeyChoice>) -> ApiResult<NewPasswordFactors> {
    let keyfile_digest = keyfile.map(|p| read_keyfile(Path::new(p))).transpose()?;
    let yubikey = match yubikey_choice {
        Some(c) => {
            let challenge: [u8; 32] = zpass_core::crypto::random_array()?;
            let response = yubikey::respond(c.slot, c.serial, &challenge)?;
            Some((
                YubiKeyFactor {
                    slot: c.slot,
                    challenge: challenge.to_vec(),
                    serial: c.serial,
                },
                response,
            ))
        }
        None => None,
    };
    Ok(NewPasswordFactors {
        keyfile_digest,
        yubikey,
    })
}
