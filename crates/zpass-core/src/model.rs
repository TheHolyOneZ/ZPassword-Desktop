use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zeroize::Zeroize;

use crate::util::now;

pub const VAULT_SCHEMA: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FieldKind {
    Text,
    Username,
    Email,
    Password,
    Url,
    Totp,
    Number,
    Date,
    Phone,
    Multiline,
    Hidden,
}

impl FieldKind {
    pub fn concealed_by_default(self) -> bool {
        matches!(self, FieldKind::Password | FieldKind::Totp | FieldKind::Hidden)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Field {
    pub id: Uuid,
    pub name: String,
    pub kind: FieldKind,
    pub value: String,
    pub concealed: bool,
}

impl Field {
    pub fn new(name: impl Into<String>, kind: FieldKind, value: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            kind,
            value: value.into(),
            concealed: kind.concealed_by_default(),
        }
    }
}

impl Drop for Field {
    fn drop(&mut self) {
        self.value.zeroize();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub field_id: Uuid,
    pub field_name: String,
    pub value: String,
    pub replaced_at: i64,
}

impl Drop for HistoryEntry {
    fn drop(&mut self) {
        self.value.zeroize();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub id: Uuid,
    pub serial: u32,
    pub template: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub folder_id: Option<Uuid>,
    #[serde(default)]
    pub labels: Vec<Uuid>,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub fields: Vec<Field>,
    #[serde(default)]
    pub history: Vec<HistoryEntry>,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default)]
    pub trashed_at: Option<i64>,
}

impl Drop for Item {
    fn drop(&mut self) {
        self.description.zeroize();
    }
}

impl Item {
    pub fn field(&self, id: &Uuid) -> Option<&Field> {
        self.fields.iter().find(|f| &f.id == id)
    }
    pub fn first_of(&self, kind: FieldKind) -> Option<&Field> {
        self.fields.iter().find(|f| f.kind == kind && !f.value.is_empty())
    }
    pub fn field_changed_at(&self, field_id: &Uuid) -> i64 {
        self.history
            .iter()
            .filter(|h| &h.field_id == field_id)
            .map(|h| h.replaced_at)
            .max()
            .unwrap_or(self.created_at)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    pub id: Uuid,
    pub name: String,
    pub parent: Option<Uuid>,
    #[serde(default)]
    pub icon: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Label {
    pub id: Uuid,
    pub name: String,
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TemplateField {
    pub name: String,
    pub kind: FieldKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Template {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub fields: Vec<TemplateField>,
    #[serde(default)]
    pub builtin: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", default)]
pub struct VaultSettings {
    pub auto_lock_minutes: u32,
    pub clipboard_clear_seconds: u32,
    pub lock_on_os_lock: bool,
    pub lock_on_minimize: bool,
    pub backups_keep: u32,
    pub trash_retention_days: u32,
    pub password_max_age_days: u32,
}

impl Default for VaultSettings {
    fn default() -> Self {
        Self {
            auto_lock_minutes: 10,
            clipboard_clear_seconds: 30,
            lock_on_os_lock: true,
            lock_on_minimize: false,
            backups_keep: 10,
            trash_retention_days: 30,
            password_max_age_days: 365,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TotpAlgorithm {
    Sha1,
    Sha256,
    Sha512,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TotpConfig {
    pub secret: Vec<u8>,
    pub algorithm: TotpAlgorithm,
    pub digits: u32,
    pub period: u64,
    #[serde(default)]
    pub last_step: u64,
}

impl Drop for TotpConfig {
    fn drop(&mut self) {
        self.secret.zeroize();
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SecurityConfig {
    pub totp: Option<TotpConfig>,
    pub pin: Option<SecretString>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SecretString(pub String);

impl std::fmt::Debug for SecretString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretString(<redacted>)")
    }
}

impl Drop for SecretString {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Vault {
    pub schema: u32,
    pub name: String,
    #[serde(default)]
    pub items: Vec<Item>,
    #[serde(default)]
    pub folders: Vec<Folder>,
    #[serde(default)]
    pub labels: Vec<Label>,
    #[serde(default)]
    pub templates: Vec<Template>,
    #[serde(default)]
    pub settings: VaultSettings,
    #[serde(default)]
    pub security: SecurityConfig,
    #[serde(default = "one")]
    pub next_serial: u32,
}

fn one() -> u32 {
    1
}

impl Vault {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            schema: VAULT_SCHEMA,
            name: name.into(),
            items: vec![],
            folders: vec![],
            labels: vec![],
            templates: vec![],
            settings: VaultSettings::default(),
            security: SecurityConfig::default(),
            next_serial: 1,
        }
    }

    pub fn take_serial(&mut self) -> u32 {
        let s = self.next_serial;
        self.next_serial += 1;
        s
    }

    pub fn item(&self, id: &Uuid) -> Option<&Item> {
        self.items.iter().find(|i| &i.id == id)
    }
    pub fn item_mut(&mut self, id: &Uuid) -> Option<&mut Item> {
        self.items.iter_mut().find(|i| &i.id == id)
    }

    pub fn purge_expired_trash(&mut self) -> usize {
        let days = self.settings.trash_retention_days;
        if days == 0 {
            return 0;
        }
        let cutoff = now() - days as i64 * 86_400;
        let before = self.items.len();
        self.items.retain(|i| i.trashed_at.is_none_or(|t| t > cutoff));
        before - self.items.len()
    }

    pub fn folder_subtree(&self, root: &Uuid) -> Vec<Uuid> {
        let mut out = vec![*root];
        let mut i = 0;
        while i < out.len() {
            let cur = out[i];
            out.extend(self.folders.iter().filter(|f| f.parent == Some(cur)).map(|f| f.id));
            i += 1;
        }
        out
    }
}
