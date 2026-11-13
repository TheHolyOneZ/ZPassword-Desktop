use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceBinding {
    pub slot_id: Uuid,
    pub backend: String,
    #[serde(default)]
    pub challenge: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppConfig {
    pub last_vault: Option<PathBuf>,
    pub recent: Vec<PathBuf>,
    pub devices: HashMap<Uuid, DeviceBinding>,
    pub reduce_motion: bool,
}

impl AppConfig {
    pub fn load(path: &Path) -> Self {
        std::fs::read(path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) {
        if let Ok(bytes) = serde_json::to_vec_pretty(self) {
            let _ = zpass_core::storage::atomic_write(path, &bytes);
        }
    }

    pub fn touch_recent(&mut self, vault: &Path) {
        self.recent.retain(|p| p != vault);
        self.recent.insert(0, vault.to_path_buf());
        self.recent.truncate(8);
        self.last_vault = Some(vault.to_path_buf());
    }
}
