use challenge_response::ChallengeResponse;
use challenge_response::config::{Config, Mode, Slot};
use serde::Serialize;
use zeroize::Zeroizing;

use crate::error::{ApiError, ApiResult};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YubiKeyDevice {
    pub name: Option<String>,
    pub serial: Option<u32>,
}

fn err(e: impl std::fmt::Debug) -> ApiError {
    ApiError::new(
        "yubikey",
        format!("YubiKey error: {e:?}. Is the key plugged in and configured for HMAC-SHA1 challenge-response?"),
    )
}

pub fn list() -> ApiResult<Vec<YubiKeyDevice>> {
    let mut cr = ChallengeResponse::new().map_err(err)?;
    match cr.find_all_devices() {
        Ok(devs) => Ok(devs
            .into_iter()
            .map(|d| YubiKeyDevice {
                name: d.name,
                serial: d.serial,
            })
            .collect()),
        Err(_) => Ok(vec![]),
    }
}

pub fn respond(slot: u8, serial: Option<u32>, challenge: &[u8]) -> ApiResult<Zeroizing<Vec<u8>>> {
    let mut cr = ChallengeResponse::new().map_err(err)?;
    let device = match serial {
        Some(s) => cr.find_device_from_serial(s).or_else(|_| cr.find_device()),
        None => cr.find_device(),
    }
    .map_err(|_| ApiError::new("yubikey", "No YubiKey found. Insert your key and try again."))?;
    let slot = Slot::from_int(slot as usize).ok_or_else(|| ApiError::invalid("YubiKey slot must be 1 or 2."))?;
    let config = Config::new_from(device)
        .set_variable_size(true)
        .set_mode(Mode::Sha1)
        .set_slot(slot);
    let hmac = cr.challenge_response_hmac(challenge, config).map_err(err)?;
    Ok(Zeroizing::new(hmac.0.to_vec()))
}
