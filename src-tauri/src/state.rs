use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use zpass_core::model::TotpConfig;
use zpass_core::pin::PinWrap;
use zpass_core::storage::VaultLock;
use zpass_core::vault::{LockedVault, OpenVault, UnlockedVia};

use crate::config::AppConfig;
use crate::error::{ApiError, ApiResult};
use crate::platform::clipboard::ClipboardService;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Stage {
    NoVault,
    Locked,
    AwaitingTotp,
    NeedsPasswordReset,
    Unlocked,
}

pub struct Session {
    pub path: Option<PathBuf>,
    pub file_lock: Option<VaultLock>,
    pub locked: Option<LockedVault>,
    pub open: Option<OpenVault>,
    pub stage: Stage,
    pub must_reset: bool,
    pub pin: Option<PinWrap>,
    pub pending_totp: Option<TotpConfig>,
    pub pending_import: Option<zpass_import::ImportBatch>,
    pub totp_failures: u32,
    pub failed_unlocks: u32,
    pub retry_after: Option<Instant>,
    pub last_activity: Instant,
    pub last_backup: Option<Instant>,
}

impl Default for Session {
    fn default() -> Self {
        Self {
            path: None,
            file_lock: None,
            locked: None,
            open: None,
            stage: Stage::NoVault,
            must_reset: false,
            pin: None,
            pending_totp: None,
            pending_import: None,
            totp_failures: 0,
            failed_unlocks: 0,
            retry_after: None,
            last_activity: Instant::now(),
            last_backup: None,
        }
    }
}

const BACKUP_INTERVAL: Duration = Duration::from_secs(15 * 60);

impl Session {
    pub fn open_ref(&self) -> ApiResult<&OpenVault> {
        match (self.stage, self.open.as_ref()) {
            (Stage::Unlocked, Some(v)) => Ok(v),
            _ => Err(ApiError::new("locked", "The vault is locked.")),
        }
    }

    pub fn open_mut(&mut self) -> ApiResult<&mut OpenVault> {
        match (self.stage, self.open.as_mut()) {
            (Stage::Unlocked, Some(v)) => Ok(v),
            _ => Err(ApiError::new("locked", "The vault is locked.")),
        }
    }

    pub fn open_for_reset(&mut self) -> ApiResult<&mut OpenVault> {
        match (self.stage, self.open.as_mut()) {
            (Stage::Unlocked | Stage::NeedsPasswordReset, Some(v)) => Ok(v),
            _ => Err(ApiError::new("locked", "The vault is locked.")),
        }
    }

    pub fn persist(&mut self) -> ApiResult<()> {
        let backup = self.last_backup.is_none_or(|t| t.elapsed() >= BACKUP_INTERVAL);
        let v = self.open.as_ref().ok_or_else(|| ApiError::state("No vault is open."))?;
        v.save_opts(backup)?;
        if backup {
            self.last_backup = Some(Instant::now());
        }
        Ok(())
    }

    pub fn enter_open(&mut self, open: OpenVault, via: UnlockedVia) {
        let needs_totp = open.data.security.totp.is_some()
            && !matches!(via, UnlockedVia::RecoveryCode { .. } | UnlockedVia::MasterKey);
        self.must_reset = matches!(via, UnlockedVia::RecoveryCode { .. } | UnlockedVia::RecoveryQuestions);
        self.open = Some(open);
        self.locked = None;
        self.failed_unlocks = 0;
        self.retry_after = None;
        self.totp_failures = 0;
        self.last_backup = None;
        self.last_activity = Instant::now();
        if needs_totp {
            self.stage = Stage::AwaitingTotp;
        } else {
            self.advance_after_totp();
        }
    }

    pub fn advance_after_totp(&mut self) {
        if self.must_reset {
            self.stage = Stage::NeedsPasswordReset;
        } else {
            self.stage = Stage::Unlocked;
            self.ensure_pin_wrap();
        }
    }

    pub fn ensure_pin_wrap(&mut self) {
        if self.pin.as_ref().is_some_and(|p| !p.is_exhausted()) {
            return;
        }
        self.pin = self.open.as_ref().and_then(|o| {
            o.data
                .security
                .pin
                .as_ref()
                .and_then(|p| PinWrap::new(&p.0, o.master_key()).ok())
        });
    }

    pub fn lock(&mut self) -> bool {
        if self.open.is_none() {
            return false;
        }
        self.open = None;
        self.pending_totp = None;
        self.pending_import = None;
        self.must_reset = false;
        self.stage = Stage::Locked;
        if let Some(p) = &self.path {
            self.locked = LockedVault::load(p).ok();
        }
        if self.locked.is_none() {
            self.stage = Stage::NoVault;
            self.file_lock = None;
            self.path = None;
        }
        true
    }

    pub fn close(&mut self) {
        self.lock();
        *self = Session::default();
    }

    pub fn register_failure(&mut self) {
        self.failed_unlocks += 1;
        if self.failed_unlocks >= 3 {
            let secs = (2u64 << (self.failed_unlocks - 3).min(4)).min(30);
            self.retry_after = Some(Instant::now() + Duration::from_secs(secs));
        }
    }

    pub fn check_throttle(&self) -> ApiResult<()> {
        if let Some(t) = self.retry_after {
            let now = Instant::now();
            if t > now {
                let secs = (t - now).as_secs() + 1;
                return Err(ApiError::new(
                    "throttled",
                    format!("Too many attempts. Try again in {secs}s."),
                ));
            }
        }
        Ok(())
    }
}

pub struct AppState {
    pub session: Mutex<Session>,
    pub config: Mutex<AppConfig>,
    pub config_path: PathBuf,
    pub default_vault: PathBuf,
    pub clipboard: ClipboardService,
}

impl AppState {
    pub fn session(&self) -> std::sync::MutexGuard<'_, Session> {
        self.session.lock().unwrap_or_else(|p| p.into_inner())
    }
    pub fn config(&self) -> std::sync::MutexGuard<'_, AppConfig> {
        self.config.lock().unwrap_or_else(|p| p.into_inner())
    }
    pub fn save_config(&self) {
        self.config().save(&self.config_path);
    }
}

pub fn lock_and_emit<R: Runtime>(app: &AppHandle<R>, reason: &str) {
    let state = app.state::<AppState>();
    let did = state.session().lock();
    if did {
        state.clipboard.clear_now();
        let _ = app.emit("session-locked", reason);
    }
}
