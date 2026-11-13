use std::path::PathBuf;
use std::time::Instant;

use tauri::{AppHandle, Runtime, State};
use zeroize::Zeroizing;
use zpass_core::crypto::{self, KdfParams};
use zpass_core::slots::SlotKind;
use zpass_core::storage::VaultLock;
use zpass_core::totp;
use zpass_core::vault::{Credential, LockedVault, OpenVault, UnlockedVia};

use super::{YubiKeyChoice, new_factors, read_keyfile};
use crate::dto::{RecentVault, Status, UnlockInfo, YubiKeyRequirement, file_label};
use crate::error::{ApiError, ApiResult};
use crate::platform::{os_unlock, yubikey};
use crate::state::{AppState, Session, Stage, lock_and_emit};

fn unlock_info(state: &AppState, s: &Session) -> Option<UnlockInfo> {
    let header = match (&s.locked, &s.open) {
        (Some(l), _) => l.header().clone(),
        (None, Some(o)) => o.header.clone(),
        _ => return None,
    };
    let mut info = UnlockInfo {
        keyfile: false,
        yubikey: None,
        recovery_codes: 0,
        recovery_questions: None,
        device_unlock: None,
        pin_attempts: s.pin.as_ref().filter(|p| !p.is_exhausted()).map(|p| p.attempts_left()),
    };
    for slot in &header.slots {
        match &slot.kind {
            SlotKind::Password { keyfile, yubikey } => {
                info.keyfile = *keyfile;
                info.yubikey = yubikey.as_ref().map(|y| YubiKeyRequirement {
                    slot: y.slot,
                    serial: y.serial,
                });
            }
            SlotKind::RecoveryCode { .. } => info.recovery_codes += 1,
            SlotKind::RecoveryQuestions { questions } => info.recovery_questions = Some(questions.clone()),
            SlotKind::OsKeystore { .. } => {}
        }
    }
    let cfg = state.config();
    if let Some(b) = cfg.devices.get(&header.vault_id)
        && header.slots.iter().any(|s| s.id == b.slot_id)
    {
        info.device_unlock = Some(b.backend.clone());
    }
    Some(info)
}

pub fn build_status(state: &AppState) -> Status {
    let s = state.session();
    let recent = state
        .config()
        .recent
        .iter()
        .map(|p| RecentVault {
            path: p.clone(),
            name: file_label(p),
            exists: p.exists(),
        })
        .collect();
    Status {
        stage: s.stage,
        vault_path: s.path.clone(),
        vault_label: s
            .open
            .as_ref()
            .map(|o| o.data.name.clone())
            .or_else(|| s.path.as_deref().map(file_label)),
        default_vault_path: state.default_vault.clone(),
        unlock: unlock_info(state, &s),
        must_reset: s.must_reset,
        recent,
        platform: std::env::consts::OS,
        device_unlock_backend: os_unlock::backend(),
    }
}

#[tauri::command]
pub fn app_status(state: State<AppState>) -> Status {
    build_status(&state)
}

#[tauri::command]
pub fn activity(state: State<AppState>) {
    state.session().last_activity = Instant::now();
}

fn select_path(state: &AppState, path: PathBuf) -> ApiResult<()> {
    let lock = VaultLock::acquire(&path)?;
    let locked = LockedVault::load(&path)?;
    let mut s = state.session();
    s.close();
    s.file_lock = Some(lock);
    s.locked = Some(locked);
    s.path = Some(path.clone());
    s.stage = Stage::Locked;
    drop(s);
    state.config().touch_recent(&path);
    state.save_config();
    Ok(())
}

#[tauri::command(async)]
pub fn vault_select(state: State<AppState>, path: PathBuf) -> ApiResult<Status> {
    if !path.exists() {
        return Err(ApiError::new("notFound", "That vault file no longer exists."));
    }
    select_path(&state, path)?;
    Ok(build_status(&state))
}

#[tauri::command(async)]
pub fn vault_close(state: State<AppState>) -> Status {
    state.session().close();
    state.clipboard.clear_now();
    build_status(&state)
}

#[tauri::command(async)]
pub fn vault_forget_recent(state: State<AppState>, path: PathBuf) -> Status {
    {
        let mut cfg = state.config();
        cfg.recent.retain(|p| p != &path);
        if cfg.last_vault.as_ref() == Some(&path) {
            cfg.last_vault = None;
        }
    }
    state.save_config();
    build_status(&state)
}

#[tauri::command(async)]
pub fn vault_create(
    state: State<AppState>,
    path: PathBuf,
    name: String,
    password: Zeroizing<String>,
    keyfile: Option<String>,
    yubikey: Option<YubiKeyChoice>,
) -> ApiResult<Status> {
    if path.exists() {
        return Err(ApiError::invalid("A file already exists at that location."));
    }
    if password.chars().count() < 10 {
        return Err(ApiError::invalid("Use at least 10 characters for the master password."));
    }
    let lock = VaultLock::acquire(&path)?;
    let factors = new_factors(keyfile.as_deref(), yubikey)?;
    let kdf = crypto::calibrate(900);
    let name = if name.trim().is_empty() {
        "My Vault".to_string()
    } else {
        name.trim().to_string()
    };
    let vault = OpenVault::create(&path, &name, password.as_bytes(), factors, kdf)?;
    vault.save()?;
    {
        let mut s = state.session();
        s.close();
        s.path = Some(path.clone());
        s.file_lock = Some(lock);
        s.enter_open(vault, UnlockedVia::Password);
    }
    state.config().touch_recent(&path);
    state.save_config();
    Ok(build_status(&state))
}

fn locked_snapshot(state: &AppState) -> ApiResult<LockedVault> {
    let s = state.session();
    s.check_throttle()?;
    match (s.stage, &s.locked) {
        (Stage::Locked, Some(l)) => Ok(l.clone()),
        _ => Err(ApiError::state("The vault is not waiting to be unlocked.")),
    }
}

fn finish_unlock(state: &AppState, locked: LockedVault, cred: Credential) -> ApiResult<Status> {
    let result = locked.unlock(&cred);
    {
        let mut s = state.session();
        if s.stage != Stage::Locked {
            return Err(ApiError::state("The vault state changed during unlock."));
        }
        match result {
            Ok((mut open, via)) => {
                if let UnlockedVia::RecoveryCode { slot_id } = via {
                    open.remove_slot(&slot_id)?;
                    open.save()?;
                }
                let purged = open.data.purge_expired_trash();
                s.enter_open(open, via);
                if purged > 0 {
                    let _ = s.open.as_ref().map(|o| o.save_opts(false));
                }
            }
            Err(e) => {
                s.register_failure();
                return Err(e.into());
            }
        }
    }
    Ok(build_status(state))
}

#[tauri::command(async)]
pub fn unlock_password(
    state: State<AppState>,
    password: Zeroizing<String>,
    keyfile: Option<String>,
) -> ApiResult<Status> {
    let locked = locked_snapshot(&state)?;
    let slot = locked
        .password_slot()
        .ok_or_else(|| ApiError::new("corrupt", "No password slot."))?;
    let SlotKind::Password {
        keyfile: needs_keyfile,
        yubikey: yk,
    } = &slot.kind
    else {
        unreachable!()
    };
    let keyfile_digest = match (needs_keyfile, keyfile) {
        (true, Some(p)) => Some(read_keyfile(std::path::Path::new(&p))?),
        (true, None) => return Err(ApiError::invalid("This vault also requires its keyfile.")),
        (false, _) => None,
    };
    let yubikey_response = match yk {
        Some(f) => Some(yubikey::respond(f.slot, f.serial, &f.challenge)?),
        None => None,
    };
    let cred = Credential::Password {
        password: password.as_bytes(),
        keyfile_digest,
        yubikey_response,
    };
    finish_unlock(&state, locked, cred)
}

#[tauri::command(async)]
pub fn unlock_recovery_code(state: State<AppState>, code: Zeroizing<String>) -> ApiResult<Status> {
    let locked = locked_snapshot(&state)?;
    finish_unlock(&state, locked, Credential::RecoveryCode(&code))
}

#[tauri::command(async)]
pub fn unlock_recovery_questions(state: State<AppState>, answers: Vec<String>) -> ApiResult<Status> {
    let locked = locked_snapshot(&state)?;
    let answers: Vec<String> = answers.into_iter().collect();
    let r = finish_unlock(&state, locked, Credential::RecoveryQuestions(&answers));
    let mut answers = answers;
    answers.iter_mut().for_each(zeroize::Zeroize::zeroize);
    r
}

#[tauri::command(async)]
pub fn unlock_device(state: State<AppState>) -> ApiResult<Status> {
    let locked = locked_snapshot(&state)?;
    let vault_id = locked.header().vault_id;
    let binding = state
        .config()
        .devices
        .get(&vault_id)
        .cloned()
        .ok_or_else(|| ApiError::new("notFound", "This device is not enrolled for quick unlock."))?;
    let secret = os_unlock::retrieve(&vault_id, &binding.slot_id, binding.challenge.as_deref())?;
    finish_unlock(
        &state,
        locked,
        Credential::OsSecret {
            slot_id: binding.slot_id,
            secret: &secret,
        },
    )
}

#[tauri::command(async)]
pub fn unlock_pin(state: State<AppState>, pin: Zeroizing<String>) -> ApiResult<Status> {
    let (locked, vmk) = {
        let mut s = state.session();
        if s.stage != Stage::Locked {
            return Err(ApiError::state("The vault is not waiting to be unlocked."));
        }
        let locked = s.locked.clone().ok_or_else(|| ApiError::state("No vault selected."))?;
        let wrap = s
            .pin
            .as_mut()
            .ok_or_else(|| ApiError::new("notFound", "Quick unlock is not armed."))?;
        match wrap.try_unwrap(&pin) {
            Ok(k) => (locked, k),
            Err(e) => {
                if wrap.is_exhausted() {
                    s.pin = None;
                    return Err(ApiError::new(
                        "pinExhausted",
                        "Too many wrong PINs. Unlock with your master password.",
                    ));
                }
                return Err(e.into());
            }
        }
    };
    finish_unlock(&state, locked, Credential::MasterKey(vmk))
}

#[tauri::command(async)]
pub fn verify_totp<R: Runtime>(app: AppHandle<R>, state: State<AppState>, code: String) -> ApiResult<Status> {
    {
        let mut s = state.session();
        if s.stage != Stage::AwaitingTotp {
            return Err(ApiError::state("No authenticator code is expected right now."));
        }
        let open = s.open.as_mut().ok_or_else(|| ApiError::state("No vault open."))?;
        let cfg = open
            .data
            .security
            .totp
            .as_ref()
            .ok_or_else(|| ApiError::state("TOTP not configured."))?;
        let now = zpass_core::util::now() as u64;
        match totp::verify(cfg, &code, now) {
            Some(step) => {
                if let Some(t) = open.data.security.totp.as_mut() {
                    t.last_step = step;
                }
                open.save_opts(false)?;
                s.totp_failures = 0;
                s.advance_after_totp();
            }
            None => {
                s.totp_failures += 1;
                if s.totp_failures >= 5 {
                    drop(s);
                    lock_and_emit(&app, "totpFailures");
                    return Err(ApiError::new(
                        "invalidCredentials",
                        "Too many wrong codes. The vault was locked.",
                    ));
                }
                return Err(ApiError::new("invalidCredentials", "That code is not valid."));
            }
        }
    }
    Ok(build_status(&state))
}

#[tauri::command(async)]
pub fn lock<R: Runtime>(app: AppHandle<R>, state: State<AppState>) -> Status {
    lock_and_emit(&app, "manual");
    build_status(&state)
}

#[tauri::command(async)]
pub fn master_password_set(
    state: State<AppState>,
    current: Option<Zeroizing<String>>,
    password: Zeroizing<String>,
    keyfile: Option<String>,
    current_keyfile: Option<String>,
    yubikey: Option<YubiKeyChoice>,
) -> ApiResult<Status> {
    if password.chars().count() < 10 {
        return Err(ApiError::invalid("Use at least 10 characters for the master password."));
    }
    let (header, stage) = {
        let mut s = state.session();
        let v = s.open_for_reset()?;
        (v.header.clone(), s.stage)
    };
    if stage == Stage::Unlocked {
        let current = current.ok_or_else(|| ApiError::invalid("Enter your current master password."))?;
        let slot = header
            .slots
            .iter()
            .find(|s| matches!(s.kind, SlotKind::Password { .. }));
        let (kf, yk) = match slot.map(|s| &s.kind) {
            Some(SlotKind::Password { keyfile, yubikey }) => (*keyfile, yubikey.clone()),
            _ => (false, None),
        };
        let keyfile_digest = if kf {
            Some(read_keyfile(std::path::Path::new(
                current_keyfile
                    .as_deref()
                    .ok_or_else(|| ApiError::invalid("Select your current keyfile."))?,
            ))?)
        } else {
            None
        };
        let yubikey_response = yk
            .map(|f| yubikey::respond(f.slot, f.serial, &f.challenge))
            .transpose()?;
        let cred = Credential::Password {
            password: current.as_bytes(),
            keyfile_digest,
            yubikey_response,
        };
        zpass_core::vault::unwrap_from_header(&header, &cred)
            .map_err(|_| ApiError::new("invalidCredentials", "The current master password is incorrect."))?;
    }
    let factors = new_factors(keyfile.as_deref(), yubikey)?;
    let kdf = header
        .slots
        .iter()
        .find_map(|s| matches!(s.kind, SlotKind::Password { .. }).then_some(s.kdf).flatten())
        .unwrap_or_else(KdfParams::strong);
    {
        let mut s = state.session();
        let v = s.open_for_reset()?;
        v.set_password(password.as_bytes(), factors, kdf)?;
        s.persist()?;
        if s.stage == Stage::NeedsPasswordReset {
            s.must_reset = false;
            s.advance_after_totp();
        }
    }
    Ok(build_status(&state))
}
