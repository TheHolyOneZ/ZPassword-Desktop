mod commands;
mod config;
mod dto;
mod error;
mod platform;
mod state;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::{Emitter, Manager};

use crate::config::AppConfig;
use crate::platform::clipboard::ClipboardService;
use crate::state::{AppState, Session, Stage, lock_and_emit};

fn spawn_idle_watch(app: tauri::AppHandle) {
    std::thread::Builder::new()
        .name("zpass-idle".into())
        .spawn(move || {
            loop {
                std::thread::sleep(Duration::from_secs(5));
                let state = app.state::<AppState>();
                let expired = {
                    let s = state.session();
                    match (s.stage, s.open.as_ref()) {
                        (Stage::Unlocked | Stage::AwaitingTotp | Stage::NeedsPasswordReset, Some(v)) => {
                            let mins = v.data.settings.auto_lock_minutes;
                            mins > 0 && s.last_activity.elapsed() >= Duration::from_secs(mins as u64 * 60)
                        }
                        _ => false,
                    }
                };
                if expired {
                    lock_and_emit(&app, "idle");
                }
            }
        })
        .expect("spawn idle watcher");
}

pub(crate) fn handler<R: tauri::Runtime>() -> impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        commands::session::app_status,
        commands::session::activity,
        commands::session::vault_select,
        commands::session::vault_close,
        commands::session::vault_forget_recent,
        commands::session::vault_create,
        commands::session::unlock_password,
        commands::session::unlock_recovery_code,
        commands::session::unlock_recovery_questions,
        commands::session::unlock_device,
        commands::session::unlock_pin,
        commands::session::verify_totp,
        commands::session::lock,
        commands::session::master_password_set,
        commands::security::security_overview,
        commands::security::recovery_codes_generate,
        commands::security::recovery_codes_clear,
        commands::security::recovery_questions_set,
        commands::security::recovery_questions_clear,
        commands::security::totp_enroll_begin,
        commands::security::totp_enroll_confirm,
        commands::security::totp_disable,
        commands::security::pin_set,
        commands::security::pin_clear,
        commands::security::device_unlock_enable,
        commands::security::device_unlock_disable,
        commands::security::slot_remove,
        commands::security::yubikey_list,
        commands::security::keyfile_generate,
        commands::items::vault_snapshot,
        commands::items::item_get,
        commands::items::item_reveal,
        commands::items::history_reveal,
        commands::items::item_copy,
        commands::items::clipboard_copy,
        commands::items::item_totp,
        commands::items::item_create,
        commands::items::item_update,
        commands::items::item_set_favorite,
        commands::items::items_move,
        commands::items::items_trash,
        commands::items::items_restore,
        commands::items::items_delete_forever,
        commands::items::trash_empty,
        commands::items::item_duplicate,
        commands::items::folder_create,
        commands::items::folder_update,
        commands::items::folder_delete,
        commands::items::label_save,
        commands::items::label_delete,
        commands::items::template_save,
        commands::items::template_delete,
        commands::items::settings_update,
        commands::tools::generate_password,
        commands::tools::password_strength,
        commands::tools::health_report,
        commands::tools::backups_list,
        commands::tools::backup_restore,
        commands::tools::export_text,
        commands::transfer::import_preview,
        commands::transfer::import_commit,
        commands::transfer::import_cancel,
        commands::transfer::export_vault,
    ]
}

#[cfg(test)]
mod e2e;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    platform::hardening::apply();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&config_dir)?;
            std::fs::create_dir_all(&data_dir)?;
            let config_path = config_dir.join("config.json");
            let config = AppConfig::load(&config_path);

            let handle = app.handle().clone();
            let clipboard = ClipboardService::start(move || {
                let _ = handle.emit("clipboard-cleared", ());
            });

            let state = AppState {
                session: Mutex::new(Session::default()),
                config: Mutex::new(config.clone()),
                config_path,
                default_vault: data_dir.join("vault.zpv"),
                clipboard,
            };
            app.manage(state);

            if let Some(last) = config.last_vault.filter(|p| p.exists()) {
                let st = app.state::<AppState>();
                if let (Ok(lock), Ok(locked)) = (
                    zpass_core::storage::VaultLock::acquire(&last),
                    zpass_core::vault::LockedVault::load(&last),
                ) {
                    let mut s = st.session();
                    s.file_lock = Some(lock);
                    s.locked = Some(locked);
                    s.path = Some(last);
                    s.stage = Stage::Locked;
                }
            }

            spawn_idle_watch(app.handle().clone());
            let handle = app.handle().clone();
            platform::os_lock::watch(Arc::new(move |reason| {
                let lock_on_os = {
                    let st = handle.state::<AppState>();
                    let s = st.session();
                    s.open.as_ref().is_some_and(|o| o.data.settings.lock_on_os_lock)
                };
                if lock_on_os {
                    lock_and_emit(&handle, reason);
                }
            }));
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Resized(_) = event
                && window.is_minimized().unwrap_or(false)
            {
                let app = window.app_handle();
                let st = app.state::<AppState>();
                let lock = st
                    .session()
                    .open
                    .as_ref()
                    .is_some_and(|o| o.data.settings.lock_on_minimize);
                if lock {
                    lock_and_emit(app, "minimize");
                }
            }
        })
        .invoke_handler(handler())
        .run(tauri::generate_context!())
        .expect("error while running ZPassword");
}
