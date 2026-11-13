use std::io::Cursor;

use keepass::db::GroupRef;
use keepass::{Database, DatabaseKey};
use zpass_core::model::FieldKind;

use crate::{ImportBatch, ImportError, ImportedItem, Result, Source, fmt_err};

const STANDARD: [&str; 5] = ["Title", "UserName", "Password", "URL", "Notes"];

fn walk(group: GroupRef<'_>, path: &mut Vec<String>, out: &mut ImportBatch, recycle: Option<uuid::Uuid>) {
    if Some(group.id().uuid()) == recycle {
        return;
    }
    for e in group.entries() {
        let mut it = ImportedItem::login(e.get_title().unwrap_or(""));
        let user = e.get_username().unwrap_or("");
        it.push(
            "Username",
            if user.contains('@') {
                FieldKind::Email
            } else {
                FieldKind::Username
            },
            user,
        );
        it.push("Password", FieldKind::Password, e.get_password().unwrap_or(""));
        it.push("Website", FieldKind::Url, e.get_url().unwrap_or(""));
        if let Some(otp) = e.get_raw_otp_value() {
            it.push("One-time code", FieldKind::Totp, otp);
        }
        it.description = e.get("Notes").unwrap_or("").to_string();
        let mut extra: Vec<(&String, &keepass::db::Value<String>)> = e
            .fields
            .iter()
            .filter(|(k, _)| !STANDARD.contains(&k.as_str()) && k.as_str() != "otp" && !k.starts_with("TimeOtp-"))
            .collect();
        extra.sort_by(|a, b| a.0.cmp(b.0));
        for (k, v) in extra {
            let kind = if v.is_protected() {
                FieldKind::Hidden
            } else {
                FieldKind::Text
            };
            it.push(k, kind, v.get());
        }
        it.labels = e.tags.clone();
        it.folder_path = path.clone();
        it.created_at = e.times.creation.map(|t| t.and_utc().timestamp());
        it.updated_at = e.times.last_modification.map(|t| t.and_utc().timestamp());
        it.ensure_title();
        out.items.push(it);
    }
    for g in group.groups() {
        path.push(g.name.clone());
        walk(g, path, out, recycle);
        path.pop();
    }
}

pub fn parse(bytes: &[u8], password: Option<&str>, keyfile: Option<&[u8]>) -> Result<ImportBatch> {
    let mut key = DatabaseKey::new();
    if let Some(p) = password.filter(|p| !p.is_empty()) {
        key = key.with_password(p);
    }
    if let Some(kf) = keyfile {
        key = key
            .with_keyfile(&mut Cursor::new(kf))
            .map_err(|_| fmt_err("The keyfile could not be read."))?;
    }
    if key.is_empty() {
        return Err(fmt_err("Enter the KeePass database password."));
    }
    let db = Database::open(&mut Cursor::new(bytes), key).map_err(|e| {
        let msg = e.to_string().to_lowercase();
        if msg.contains("key") || msg.contains("password") || msg.contains("hmac") || msg.contains("credential") {
            ImportError::Credentials
        } else {
            fmt_err(format!("Couldn't open this KeePass file: {e}"))
        }
    })?;
    let mut batch = ImportBatch {
        source: Some(Source::KeePass),
        ..Default::default()
    };
    let recycle = db.meta.recyclebin_uuid;
    let mut path = vec![];
    walk(db.root(), &mut path, &mut batch, recycle);
    Ok(batch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use keepass::db::Value;

    fn sample_kdbx(password: &str) -> Vec<u8> {
        let mut db = Database::new();
        {
            let mut root = db.root_mut();
            let mut e = root.add_entry();
            e.set_unprotected("Title", "Router");
            e.set_unprotected("UserName", "admin");
            e.set_protected("Password", "hunter2");
            e.set_unprotected("URL", "http://192.168.1.1");
            e.set("API token", Value::protected("tok".to_string()));
            e.tags.push("home".into());
        }
        {
            let mut root = db.root_mut();
            let mut g = root.add_group();
            g.name = "Work".into();
            let mut e = g.add_entry();
            e.set_unprotected("Title", "VPN");
            e.set_protected("Password", "vpn-pw");
        }
        let mut out = vec![];
        db.save(&mut out, DatabaseKey::new().with_password(password)).unwrap();
        out
    }

    #[test]
    fn imports_entries_groups_and_custom_fields() {
        let bytes = sample_kdbx("kp-pass");
        let b = parse(&bytes, Some("kp-pass"), None).unwrap();
        assert_eq!(b.items.len(), 2);
        let router = b.items.iter().find(|i| i.title == "Router").unwrap();
        assert!(
            router
                .fields
                .iter()
                .any(|f| f.kind == FieldKind::Password && f.value == "hunter2")
        );
        assert!(router.fields.iter().any(|f| f.name == "API token" && f.concealed));
        assert_eq!(router.labels, vec!["home"]);
        let vpn = b.items.iter().find(|i| i.title == "VPN").unwrap();
        assert_eq!(vpn.folder_path, vec!["Work"]);
    }

    #[test]
    fn wrong_password_is_reported() {
        let bytes = sample_kdbx("kp-pass");
        assert!(matches!(
            parse(&bytes, Some("nope"), None),
            Err(ImportError::Credentials)
        ));
    }
}
