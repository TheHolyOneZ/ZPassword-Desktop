pub mod bitwarden;
pub mod csv_import;
pub mod export;
pub mod kdbx;

use std::collections::HashSet;

use serde::Serialize;
use uuid::Uuid;
use zpass_core::model::{Field, FieldKind, Folder, Item, Label, Vault};
use zpass_core::util::now;

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("{0}")]
    Format(String),
    #[error("The password or keyfile for this file is wrong.")]
    Credentials,
    #[error(transparent)]
    Core(#[from] zpass_core::Error),
}

pub type Result<T> = std::result::Result<T, ImportError>;

fn fmt_err(msg: impl Into<String>) -> ImportError {
    ImportError::Format(msg.into())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Source {
    Bitwarden,
    Chrome,
    Firefox,
    Safari,
    OnePassword,
    LastPass,
    BitwardenCsv,
    GenericCsv,
    KeePass,
    ZPassword,
}

#[derive(Debug, Clone, Default)]
pub struct ImportedItem {
    pub template: String,
    pub title: String,
    pub description: String,
    pub favorite: bool,
    pub folder_path: Vec<String>,
    pub labels: Vec<String>,
    pub fields: Vec<Field>,
    pub created_at: Option<i64>,
    pub updated_at: Option<i64>,
}

impl ImportedItem {
    pub fn login(title: &str) -> Self {
        Self {
            template: "login".into(),
            title: title.trim().into(),
            ..Default::default()
        }
    }

    pub fn push(&mut self, name: &str, kind: FieldKind, value: &str) {
        if !value.trim().is_empty() {
            self.fields.push(Field::new(name, kind, value));
        }
    }

    fn first(&self, kind: FieldKind) -> &str {
        self.fields
            .iter()
            .find(|f| f.kind == kind)
            .map(|f| f.value.as_str())
            .unwrap_or("")
    }

    pub fn ensure_title(&mut self) {
        if self.title.trim().is_empty() {
            let url = self.first(FieldKind::Url);
            self.title = host(url).unwrap_or_else(|| "Untitled".into());
        }
    }
}

pub fn host(url: &str) -> Option<String> {
    let rest = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
    let h = rest
        .split(['/', '?', '#'])
        .next()?
        .rsplit('@')
        .next()?
        .split(':')
        .next()?;
    let h = h.trim_start_matches("www.");
    (!h.is_empty()).then(|| h.to_string())
}

#[derive(Debug, Default)]
pub struct ImportBatch {
    pub source: Option<Source>,
    pub items: Vec<ImportedItem>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    pub source: Option<Source>,
    pub items: usize,
    pub folders: usize,
    pub with_totp: usize,
    pub duplicates: usize,
    pub sample: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub added: usize,
    pub skipped: usize,
    pub folders_created: usize,
}

fn fingerprint_parts(title: &str, fields: &[Field]) -> String {
    let get = |k: FieldKind| {
        fields
            .iter()
            .find(|f| f.kind == k)
            .map(|f| f.value.trim())
            .unwrap_or("")
    };
    format!(
        "{}\u{1f}{}\u{1f}{}\u{1f}{}",
        title.trim().to_lowercase(),
        get(FieldKind::Username).to_lowercase(),
        get(FieldKind::Password),
        host(get(FieldKind::Url)).unwrap_or_default()
    )
}

fn existing_fingerprints(vault: &Vault) -> HashSet<String> {
    vault
        .items
        .iter()
        .filter(|i| i.trashed_at.is_none())
        .map(|i| fingerprint_parts(&i.title, &i.fields))
        .collect()
}

pub fn summarize(batch: &ImportBatch, vault: &Vault) -> ImportSummary {
    let fps = existing_fingerprints(vault);
    let folders: HashSet<&Vec<String>> = batch
        .items
        .iter()
        .filter(|i| !i.folder_path.is_empty())
        .map(|i| &i.folder_path)
        .collect();
    ImportSummary {
        source: batch.source,
        items: batch.items.len(),
        folders: folders.len(),
        with_totp: batch
            .items
            .iter()
            .filter(|i| i.fields.iter().any(|f| f.kind == FieldKind::Totp))
            .count(),
        duplicates: batch
            .items
            .iter()
            .filter(|i| fps.contains(&fingerprint_parts(&i.title, &i.fields)))
            .count(),
        sample: batch.items.iter().take(8).map(|i| i.title.clone()).collect(),
        warnings: batch.warnings.clone(),
    }
}

fn folder_for_path(vault: &mut Vault, path: &[String], created: &mut usize) -> Option<Uuid> {
    let mut parent: Option<Uuid> = None;
    for name in path.iter().map(|n| n.trim()).filter(|n| !n.is_empty()) {
        let found = vault
            .folders
            .iter()
            .find(|f| f.parent == parent && f.name.eq_ignore_ascii_case(name))
            .map(|f| f.id);
        parent = Some(match found {
            Some(id) => id,
            None => {
                let id = Uuid::new_v4();
                vault.folders.push(Folder {
                    id,
                    name: name.to_string(),
                    parent,
                    icon: None,
                    created_at: now(),
                });
                *created += 1;
                id
            }
        });
    }
    parent
}

fn label_id(vault: &mut Vault, name: &str) -> Uuid {
    if let Some(l) = vault.labels.iter().find(|l| l.name.eq_ignore_ascii_case(name.trim())) {
        return l.id;
    }
    let id = Uuid::new_v4();
    vault.labels.push(Label {
        id,
        name: name.trim().into(),
        color: "steel".into(),
    });
    id
}

pub fn merge(vault: &mut Vault, batch: ImportBatch, skip_duplicates: bool, root_folder: Option<&str>) -> ImportReport {
    let mut fps = existing_fingerprints(vault);
    let mut report = ImportReport {
        added: 0,
        skipped: 0,
        folders_created: 0,
    };
    let t = now();
    for mut it in batch.items {
        it.ensure_title();
        let fp = fingerprint_parts(&it.title, &it.fields);
        if skip_duplicates && fps.contains(&fp) {
            report.skipped += 1;
            continue;
        }
        fps.insert(fp);
        let mut path: Vec<String> = root_folder.map(|r| vec![r.to_string()]).unwrap_or_default();
        path.extend(it.folder_path.iter().cloned());
        let folder_id = folder_for_path(vault, &path, &mut report.folders_created);
        let labels = it
            .labels
            .iter()
            .filter(|l| !l.trim().is_empty())
            .map(|l| label_id(vault, l))
            .collect();
        let serial = vault.take_serial();
        vault.items.push(Item {
            id: Uuid::new_v4(),
            serial,
            template: it.template.clone(),
            title: std::mem::take(&mut it.title),
            description: std::mem::take(&mut it.description),
            folder_id,
            labels,
            favorite: it.favorite,
            fields: std::mem::take(&mut it.fields),
            history: vec![],
            created_at: it.created_at.unwrap_or(t),
            updated_at: it.updated_at.unwrap_or(t),
            trashed_at: None,
        });
        report.added += 1;
    }
    report
}

pub fn zpassword(bytes: &[u8], password: &str) -> Result<ImportBatch> {
    use zpass_core::vault::{Credential, LockedVault};
    let locked = LockedVault::from_bytes(std::path::Path::new("import.zpv"), bytes)?;
    let cred = Credential::Password {
        password: password.as_bytes(),
        keyfile_digest: None,
        yubikey_response: None,
    };
    let (open, _) = locked.unlock(&cred).map_err(|e| match e {
        zpass_core::Error::InvalidCredentials => ImportError::Credentials,
        other => other.into(),
    })?;
    let data = &open.data;
    let path_of = |mut id: Option<Uuid>| {
        let mut p = vec![];
        while let Some(fid) = id {
            match data.folders.iter().find(|f| f.id == fid) {
                Some(f) => {
                    p.insert(0, f.name.clone());
                    id = f.parent;
                }
                None => break,
            }
        }
        p
    };
    let mut batch = ImportBatch {
        source: Some(Source::ZPassword),
        ..Default::default()
    };
    for i in data.items.iter().filter(|i| i.trashed_at.is_none()) {
        batch.items.push(ImportedItem {
            template: i.template.clone(),
            title: i.title.clone(),
            description: i.description.clone(),
            favorite: i.favorite,
            folder_path: path_of(i.folder_id),
            labels: i
                .labels
                .iter()
                .filter_map(|l| data.labels.iter().find(|x| &x.id == l).map(|x| x.name.clone()))
                .collect(),
            fields: i
                .fields
                .iter()
                .map(|f| Field::new(f.name.clone(), f.kind, f.value.clone()))
                .collect(),
            created_at: Some(i.created_at),
            updated_at: Some(i.updated_at),
        });
    }
    if data
        .items
        .iter()
        .any(|i| !data.templates.is_empty() && i.template.starts_with("user:"))
    {
        batch
            .warnings
            .push("Custom templates are not copied; those items keep their fields.".into());
    }
    Ok(batch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_parsing() {
        assert_eq!(
            host("https://www.example.com/login?x=1").as_deref(),
            Some("example.com")
        );
        assert_eq!(host("user@db.internal:5432/x").as_deref(), Some("db.internal"));
        assert_eq!(host("example.org").as_deref(), Some("example.org"));
        assert_eq!(host(""), None);
    }

    #[test]
    fn merge_creates_folders_labels_and_skips_dupes() {
        let mut v = Vault::new("t");
        let mut a = ImportedItem::login("Site");
        a.push("Username", FieldKind::Username, "me");
        a.push("Password", FieldKind::Password, "pw");
        a.folder_path = vec!["Work".into(), "DB".into()];
        a.labels = vec!["prod".into()];
        let b = a.clone();
        let r = merge(
            &mut v,
            ImportBatch {
                items: vec![a, b],
                ..Default::default()
            },
            true,
            Some("Imported"),
        );
        assert_eq!(r.added, 1);
        assert_eq!(r.skipped, 1);
        assert_eq!(r.folders_created, 3);
        assert_eq!(v.labels.len(), 1);
        let again = ImportedItem {
            ..v.items[0]
                .fields
                .iter()
                .fold(ImportedItem::login("Site"), |mut acc, f| {
                    acc.push(&f.name, f.kind, &f.value);
                    acc
                })
        };
        let r2 = merge(
            &mut v,
            ImportBatch {
                items: vec![again],
                ..Default::default()
            },
            true,
            None,
        );
        assert_eq!(r2.skipped, 1);
    }
}
