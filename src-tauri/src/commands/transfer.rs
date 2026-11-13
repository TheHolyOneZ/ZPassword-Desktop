use std::path::PathBuf;

use serde::Deserialize;
use tauri::State;
use zeroize::Zeroizing;
use zpass_core::crypto;
use zpass_import::{ImportReport, ImportSummary};

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

const MAX_IMPORT: u64 = 64 * 1024 * 1024;

fn import_err(e: zpass_import::ImportError) -> ApiError {
    match e {
        zpass_import::ImportError::Credentials => ApiError::new("invalidCredentials", e.to_string()),
        zpass_import::ImportError::Core(c) => c.into(),
        other => ApiError::invalid(other.to_string()),
    }
}

#[tauri::command(async)]
pub fn import_preview(
    state: State<AppState>,
    path: PathBuf,
    password: Option<Zeroizing<String>>,
    keyfile: Option<PathBuf>,
) -> ApiResult<ImportSummary> {
    state.session().open_ref()?;
    let meta = std::fs::metadata(&path)?;
    if meta.len() > MAX_IMPORT {
        return Err(ApiError::invalid("That file is too large to import."));
    }
    let bytes = Zeroizing::new(std::fs::read(&path)?);
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let pw = password.as_deref().map(|s| s.as_str());
    let batch = match ext.as_str() {
        "kdbx" | "kdb" => {
            let kf = keyfile.map(std::fs::read).transpose()?.map(Zeroizing::new);
            zpass_import::kdbx::parse(&bytes, pw, kf.as_deref().map(|v| v.as_slice()))
        }
        "zpv" | "zpvbak" => zpass_import::zpassword(&bytes, pw.unwrap_or("")),
        "json" => zpass_import::bitwarden::parse(&bytes),
        _ if bytes.starts_with(zpass_core::format::MAGIC) => zpass_import::zpassword(&bytes, pw.unwrap_or("")),
        _ if bytes.first() == Some(&b'{') => zpass_import::bitwarden::parse(&bytes),
        _ => zpass_import::csv_import::parse(&bytes),
    }
    .map_err(import_err)?;
    let mut s = state.session();
    let summary = zpass_import::summarize(&batch, &s.open_ref()?.data);
    s.pending_import = Some(batch);
    Ok(summary)
}

#[tauri::command(async)]
pub fn import_commit(
    state: State<AppState>,
    skip_duplicates: bool,
    into_folder: Option<String>,
) -> ApiResult<ImportReport> {
    let mut s = state.session();
    let batch = s
        .pending_import
        .take()
        .ok_or_else(|| ApiError::state("Choose a file to import first."))?;
    let folder = into_folder.map(|f| f.trim().to_string()).filter(|f| !f.is_empty());
    let v = s.open_mut()?;
    let report = zpass_import::merge(&mut v.data, batch, skip_duplicates, folder.as_deref());
    s.persist()?;
    Ok(report)
}

#[tauri::command]
pub fn import_cancel(state: State<AppState>) {
    state.session().pending_import = None;
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExportFormat {
    Zpassword,
    BitwardenJson,
    Csv,
}

#[tauri::command(async)]
pub fn export_vault(
    state: State<AppState>,
    path: PathBuf,
    format: ExportFormat,
    password: Option<Zeroizing<String>>,
) -> ApiResult<()> {
    let bytes: Zeroizing<Vec<u8>> = {
        let s = state.session();
        let v = s.open_ref()?;
        match format {
            ExportFormat::Zpassword => {
                let pw = password.ok_or_else(|| ApiError::invalid("Choose a password for the backup."))?;
                if pw.chars().count() < 10 {
                    return Err(ApiError::invalid("Use at least 10 characters for the backup password."));
                }
                Zeroizing::new(v.export_encrypted(pw.as_bytes(), crypto::KdfParams::strong())?)
            }
            ExportFormat::BitwardenJson => Zeroizing::new(zpass_import::export::bitwarden_json(&v.data).into_bytes()),
            ExportFormat::Csv => Zeroizing::new(zpass_import::export::csv(&v.data).into_bytes()),
        }
    };
    zpass_core::storage::atomic_write(&path, &bytes)?;
    Ok(())
}
