use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zpass_core::model::{Field, FieldKind, Folder, Item, Label, Template, Vault, VaultSettings};
use zpass_core::slots::SlotKind;
use zpass_core::templates;

use crate::state::Stage;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentVault {
    pub path: PathBuf,
    pub name: String,
    pub exists: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YubiKeyRequirement {
    pub slot: u8,
    pub serial: Option<u32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnlockInfo {
    pub keyfile: bool,
    pub yubikey: Option<YubiKeyRequirement>,
    pub recovery_codes: usize,
    pub recovery_questions: Option<Vec<String>>,
    pub device_unlock: Option<String>,
    pub pin_attempts: Option<u8>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub stage: Stage,
    pub vault_path: Option<PathBuf>,
    pub vault_label: Option<String>,
    pub default_vault_path: PathBuf,
    pub unlock: Option<UnlockInfo>,
    pub must_reset: bool,
    pub recent: Vec<RecentVault>,
    pub platform: &'static str,
    pub device_unlock_backend: Option<&'static str>,
}

pub fn file_label(p: &std::path::Path) -> String {
    p.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Vault".into())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemSummary {
    pub id: Uuid,
    pub serial: u32,
    pub template: String,
    pub title: String,
    pub subtitle: String,
    pub url: Option<String>,
    pub folder_id: Option<Uuid>,
    pub labels: Vec<Uuid>,
    pub favorite: bool,
    pub has_totp: bool,
    pub has_password: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub trashed_at: Option<i64>,
}

impl From<&Item> for ItemSummary {
    fn from(i: &Item) -> Self {
        let first = |k: FieldKind| i.first_of(k).map(|f| f.value.clone());
        let subtitle = first(FieldKind::Username)
            .or_else(|| first(FieldKind::Email))
            .or_else(|| {
                i.fields
                    .iter()
                    .find(|f| !f.concealed && !f.value.is_empty() && f.kind != FieldKind::Url)
                    .map(|f| f.value.clone())
            })
            .unwrap_or_default();
        Self {
            id: i.id,
            serial: i.serial,
            template: i.template.clone(),
            title: i.title.clone(),
            subtitle: subtitle.lines().next().unwrap_or("").chars().take(120).collect(),
            url: first(FieldKind::Url),
            folder_id: i.folder_id,
            labels: i.labels.clone(),
            favorite: i.favorite,
            has_totp: i.first_of(FieldKind::Totp).is_some(),
            has_password: i.first_of(FieldKind::Password).is_some(),
            created_at: i.created_at,
            updated_at: i.updated_at,
            trashed_at: i.trashed_at,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldView {
    pub id: Uuid,
    pub name: String,
    pub kind: FieldKind,
    pub concealed: bool,
    pub value: Option<String>,
    pub empty: bool,
}

impl From<&Field> for FieldView {
    fn from(f: &Field) -> Self {
        Self {
            id: f.id,
            name: f.name.clone(),
            kind: f.kind,
            concealed: f.concealed,
            value: if f.concealed { None } else { Some(f.value.clone()) },
            empty: f.value.is_empty(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryView {
    pub index: usize,
    pub field_id: Uuid,
    pub field_name: String,
    pub replaced_at: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemDetail {
    pub id: Uuid,
    pub serial: u32,
    pub template: String,
    pub title: String,
    pub description: String,
    pub folder_id: Option<Uuid>,
    pub labels: Vec<Uuid>,
    pub favorite: bool,
    pub fields: Vec<FieldView>,
    pub history: Vec<HistoryView>,
    pub created_at: i64,
    pub updated_at: i64,
    pub trashed_at: Option<i64>,
}

impl From<&Item> for ItemDetail {
    fn from(i: &Item) -> Self {
        Self {
            id: i.id,
            serial: i.serial,
            template: i.template.clone(),
            title: i.title.clone(),
            description: i.description.clone(),
            folder_id: i.folder_id,
            labels: i.labels.clone(),
            favorite: i.favorite,
            fields: i.fields.iter().map(FieldView::from).collect(),
            history: i
                .history
                .iter()
                .enumerate()
                .rev()
                .map(|(index, h)| HistoryView {
                    index,
                    field_id: h.field_id,
                    field_name: h.field_name.clone(),
                    replaced_at: h.replaced_at,
                })
                .collect(),
            created_at: i.created_at,
            updated_at: i.updated_at,
            trashed_at: i.trashed_at,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub name: String,
    pub items: Vec<ItemSummary>,
    pub folders: Vec<Folder>,
    pub labels: Vec<Label>,
    pub templates: Vec<Template>,
    pub settings: VaultSettings,
}

impl From<&Vault> for Snapshot {
    fn from(v: &Vault) -> Self {
        let mut tpl = templates::builtin();
        tpl.extend(v.templates.iter().cloned());
        Self {
            name: v.name.clone(),
            items: v.items.iter().map(ItemSummary::from).collect(),
            folders: v.folders.clone(),
            labels: v.labels.clone(),
            templates: tpl,
            settings: v.settings.clone(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldInput {
    pub id: Option<Uuid>,
    pub name: String,
    pub kind: FieldKind,
    pub concealed: bool,
    pub value: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemInput {
    pub template: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub folder_id: Option<Uuid>,
    #[serde(default)]
    pub labels: Vec<Uuid>,
    #[serde(default)]
    pub favorite: bool,
    pub fields: Vec<FieldInput>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlotView {
    pub id: Uuid,
    pub kind: &'static str,
    pub created_at: i64,
    pub detail: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityOverview {
    pub keyfile: bool,
    pub yubikey: Option<YubiKeyRequirement>,
    pub recovery_codes: usize,
    pub recovery_questions: Option<Vec<String>>,
    pub totp: bool,
    pub pin: bool,
    pub device_unlock: Option<String>,
    pub device_unlock_available: bool,
    pub device_unlock_backend: Option<&'static str>,
    pub slots: Vec<SlotView>,
    pub kdf_memory_mib: u32,
    pub kdf_passes: u32,
}

pub fn slot_views(slots: &[zpass_core::slots::Slot]) -> Vec<SlotView> {
    slots
        .iter()
        .map(|s| SlotView {
            id: s.id,
            kind: s.kind.tag(),
            created_at: s.created_at,
            detail: match &s.kind {
                SlotKind::Password { keyfile, yubikey } => Some(
                    [keyfile.then_some("keyfile"), yubikey.as_ref().map(|_| "YubiKey")]
                        .into_iter()
                        .flatten()
                        .collect::<Vec<_>>()
                        .join(" + "),
                )
                .filter(|s| !s.is_empty()),
                SlotKind::RecoveryCode { index } => Some(format!("#{}", index + 1)),
                SlotKind::RecoveryQuestions { questions } => Some(format!("{} questions", questions.len())),
                SlotKind::OsKeystore { backend, device } => Some(format!("{backend} on {device}")),
            },
        })
        .collect()
}
