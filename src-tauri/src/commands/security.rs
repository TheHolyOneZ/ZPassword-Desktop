use std::path::PathBuf;

use serde::Serialize;
use tauri::State;
use zeroize::Zeroizing;
use zpass_core::crypto::KdfParams;
use zpass_core::model::SecretString;
use zpass_core::slots::SlotKind;
use zpass_core::{pin, totp};

use crate::config::DeviceBinding;
use crate::dto::{SecurityOverview, YubiKeyRequirement, slot_views};
use crate::error::{ApiError, ApiResult};
use crate::platform::{os_unlock, yubikey};
use crate::state::AppState;

#[tauri::command]
pub fn security_overview(state: State<AppState>) -> ApiResult<SecurityOverview> {
    let s = state.session();
    let v = s.open_ref()?;
    let mut o = SecurityOverview {
        keyfile: false,
        yubikey: None,
        recovery_codes: 0,
        recovery_questions: None,
        totp: v.data.security.totp.is_some(),
        pin: v.data.security.pin.is_some(),
        device_unlock: None,
        device_unlock_available: os_unlock::available(),
        device_unlock_backend: os_unlock::backend(),
        slots: slot_views(&v.header.slots),
        kdf_memory_mib: 0,
        kdf_passes: 0,
    };
    for slot in &v.header.slots {
        match &slot.kind {
            SlotKind::Password { keyfile, yubikey } => {
                o.keyfile = *keyfile;
                o.yubikey = yubikey.as_ref().map(|y| YubiKeyRequirement {
                    slot: y.slot,
                    serial: y.serial,
                });
                if let Some(k) = slot.kdf {
                    o.kdf_memory_mib = k.m_cost_kib / 1024;
                    o.kdf_passes = k.t_cost;
                }
            }
            SlotKind::RecoveryCode { .. } => o.recovery_codes += 1,
            SlotKind::RecoveryQuestions { questions } => o.recovery_questions = Some(questions.clone()),
            SlotKind::OsKeystore { .. } => {}
        }
    }
    if let Some(b) = state.config().devices.get(&v.vault_id())
        && v.header.slots.iter().any(|s| s.id == b.slot_id)
    {
        o.device_unlock = Some(b.backend.clone());
    }
    Ok(o)
}

#[tauri::command(async)]
pub fn recovery_codes_generate(state: State<AppState>) -> ApiResult<Vec<String>> {
    let mut s = state.session();
    let v = s.open_mut()?;
    let codes = v.regenerate_recovery_codes()?;
    s.persist()?;
    Ok(codes.iter().map(|c| c.to_string()).collect())
}

#[tauri::command(async)]
pub fn recovery_codes_clear(state: State<AppState>) -> ApiResult<()> {
    let mut s = state.session();
    s.open_mut()?.clear_recovery_codes();
    s.persist()
}

#[tauri::command(async)]
pub fn recovery_questions_set(state: State<AppState>, questions: Vec<String>, answers: Vec<String>) -> ApiResult<()> {
    let answers: Vec<Zeroizing<String>> = answers.into_iter().map(Zeroizing::new).collect();
    let plain: Vec<String> = answers.iter().map(|a| a.to_string()).collect();
    let questions: Vec<String> = questions.into_iter().map(|q| q.trim().to_string()).collect();
    if questions.iter().any(|q| q.is_empty()) {
        return Err(ApiError::invalid("Questions cannot be empty."));
    }
    let r = {
        let mut s = state.session();
        let v = s.open_mut()?;
        v.set_recovery_questions(questions, &plain, KdfParams::heavy())
            .map_err(ApiError::from)
            .and_then(|_| s.persist())
    };
    let mut plain = plain;
    plain.iter_mut().for_each(zeroize::Zeroize::zeroize);
    r
}

#[tauri::command(async)]
pub fn recovery_questions_clear(state: State<AppState>) -> ApiResult<()> {
    let mut s = state.session();
    s.open_mut()?.clear_recovery_questions();
    s.persist()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TotpEnrollment {
    pub secret: String,
    pub uri: String,
    pub qr_svg: String,
}

#[tauri::command(async)]
pub fn totp_enroll_begin(state: State<AppState>) -> ApiResult<TotpEnrollment> {
    let mut s = state.session();
    let name = s.open_ref()?.data.name.clone();
    let cfg = totp::new_secret()?;
    let uri = totp::to_uri(&cfg, "ZPassword", &name);
    let qr = qrcode::QrCode::with_error_correction_level(uri.as_bytes(), qrcode::EcLevel::M)
        .map_err(|_| ApiError::new("internal", "QR encoding failed"))?;
    let qr_svg = qr
        .render::<qrcode::render::svg::Color>()
        .min_dimensions(220, 220)
        .quiet_zone(false)
        .dark_color(qrcode::render::svg::Color("#0b0c0d"))
        .light_color(qrcode::render::svg::Color("#e9e6df"))
        .build();
    let secret = totp::secret_base32(&cfg)
        .as_bytes()
        .chunks(4)
        .map(|c| String::from_utf8_lossy(c).into_owned())
        .collect::<Vec<_>>()
        .join(" ");
    s.pending_totp = Some(cfg);
    Ok(TotpEnrollment { secret, uri, qr_svg })
}

#[tauri::command(async)]
pub fn totp_enroll_confirm(state: State<AppState>, code: String) -> ApiResult<()> {
    let mut s = state.session();
    s.open_ref()?;
    let mut cfg = s
        .pending_totp
        .take()
        .ok_or_else(|| ApiError::state("Start the authenticator setup first."))?;
    match totp::verify(&cfg, &code, zpass_core::util::now() as u64) {
        Some(step) => {
            cfg.last_step = step;
            s.open_mut()?.data.security.totp = Some(cfg);
            s.persist()
        }
        None => {
            s.pending_totp = Some(cfg);
            Err(ApiError::new(
                "invalidCredentials",
                "That code does not match. Check the time on your phone.",
            ))
        }
    }
}

#[tauri::command(async)]
pub fn totp_disable(state: State<AppState>, code: String) -> ApiResult<()> {
    let mut s = state.session();
    let v = s.open_mut()?;
    let cfg = v
        .data
        .security
        .totp
        .as_ref()
        .ok_or_else(|| ApiError::state("The authenticator is not enabled."))?;
    if totp::verify(cfg, &code, zpass_core::util::now() as u64).is_none() {
        return Err(ApiError::new("invalidCredentials", "That code is not valid."));
    }
    v.data.security.totp = None;
    s.persist()
}

#[tauri::command(async)]
pub fn pin_set(state: State<AppState>, pin: Zeroizing<String>) -> ApiResult<()> {
    pin::validate_pin(&pin)?;
    let mut s = state.session();
    s.open_mut()?.data.security.pin = Some(SecretString(pin.to_string()));
    s.persist()?;
    s.pin = None;
    s.ensure_pin_wrap();
    Ok(())
}

#[tauri::command(async)]
pub fn pin_clear(state: State<AppState>) -> ApiResult<()> {
    let mut s = state.session();
    s.open_mut()?.data.security.pin = None;
    s.pin = None;
    s.persist()
}

#[tauri::command(async)]
pub fn device_unlock_enable(state: State<AppState>) -> ApiResult<()> {
    let backend = os_unlock::backend().ok_or_else(|| ApiError::new("osUnlock", "Not supported on this platform."))?;
    let vault_id = state.session().open_ref()?.vault_id();
    let enrollment = os_unlock::enroll(&vault_id)?;
    let device = gethostname::gethostname().to_string_lossy().into_owned();
    let slot_id = {
        let mut s = state.session();
        let v = s.open_mut()?;
        let old = state.config().devices.get(&vault_id).map(|b| b.slot_id);
        if let Some(old) = old {
            let _ = v.remove_slot(&old);
        }
        let id = v.add_os_slot(backend, &device, &enrollment.secret)?;
        os_unlock::store(&vault_id, &id, &enrollment)?;
        s.persist()?;
        id
    };
    state.config().devices.insert(
        vault_id,
        DeviceBinding {
            slot_id,
            backend: backend.into(),
            challenge: enrollment.challenge,
        },
    );
    state.save_config();
    Ok(())
}

#[tauri::command(async)]
pub fn device_unlock_disable(state: State<AppState>) -> ApiResult<()> {
    let mut s = state.session();
    let v = s.open_mut()?;
    let vault_id = v.vault_id();
    let binding = state.config().devices.remove(&vault_id);
    if let Some(b) = binding {
        let _ = v.remove_slot(&b.slot_id);
        os_unlock::forget(&vault_id, &b.slot_id);
        s.persist()?;
    }
    drop(s);
    state.save_config();
    Ok(())
}

#[tauri::command(async)]
pub fn slot_remove(state: State<AppState>, slot_id: uuid::Uuid) -> ApiResult<()> {
    let mut s = state.session();
    s.open_mut()?.remove_slot(&slot_id)?;
    s.persist()
}

#[tauri::command(async)]
pub fn yubikey_list() -> ApiResult<Vec<yubikey::YubiKeyDevice>> {
    yubikey::list()
}

#[tauri::command(async)]
pub fn keyfile_generate(path: PathBuf) -> ApiResult<()> {
    if path.exists() {
        return Err(ApiError::invalid("Refusing to overwrite an existing file."));
    }
    let bytes: [u8; 64] = zpass_core::crypto::random_array()?;
    zpass_core::storage::atomic_write(&path, &bytes)?;
    Ok(())
}
