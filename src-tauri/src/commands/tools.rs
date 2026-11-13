use std::path::PathBuf;

use tauri::{AppHandle, Runtime, State};
use zpass_core::generator::{self, Generated, GeneratorOptions};
use zpass_core::health::{self, HealthReport, Strength};
use zpass_core::storage::{self, BackupInfo};
use zpass_core::vault::LockedVault;

use crate::error::{ApiError, ApiResult};
use crate::state::{AppState, Stage, lock_and_emit};

#[tauri::command]
pub fn generate_password(options: GeneratorOptions) -> ApiResult<Generated> {
    Ok(generator::generate(&options)?)
}

#[tauri::command]
pub fn password_strength(password: zeroize::Zeroizing<String>) -> Strength {
    health::strength(&password, &[])
}

#[tauri::command(async)]
pub fn health_report(state: State<AppState>) -> ApiResult<HealthReport> {
    let s = state.session();
    Ok(health::report(&s.open_ref()?.data))
}

#[tauri::command(async)]
pub fn backups_list(state: State<AppState>) -> ApiResult<Vec<BackupInfo>> {
    let s = state.session();
    let path = s.path.clone().ok_or_else(|| ApiError::state("No vault selected."))?;
    Ok(storage::list_backups(&path)?)
}

#[tauri::command(async)]
pub fn backup_restore<R: Runtime>(app: AppHandle<R>, state: State<AppState>, backup: PathBuf) -> ApiResult<()> {
    let path = {
        let s = state.session();
        if s.stage != Stage::Unlocked {
            return Err(ApiError::new("locked", "Unlock the vault to restore a backup."));
        }
        s.path.clone().ok_or_else(|| ApiError::state("No vault selected."))?
    };
    let dir = storage::backup_dir(&path);
    let canonical = backup.canonicalize()?;
    if !canonical.starts_with(dir.canonicalize()?) {
        return Err(ApiError::invalid("That file is not one of this vault's backups."));
    }
    let bytes = std::fs::read(&canonical)?;
    LockedVault::from_bytes(&canonical, &bytes)?;
    storage::rotate_backup(&path, 100)?;
    storage::atomic_write(&path, &bytes)?;
    lock_and_emit(&app, "restored");
    Ok(())
}

#[tauri::command(async)]
pub fn export_text(state: State<AppState>, path: PathBuf, text: zeroize::Zeroizing<String>) -> ApiResult<()> {
    state.session().open_ref()?;
    storage::atomic_write(&path, text.as_bytes())?;
    Ok(())
}
