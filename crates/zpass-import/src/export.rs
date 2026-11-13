use serde_json::{Value, json};
use uuid::Uuid;
use zpass_core::model::{FieldKind, Item, Vault};

fn folder_path(v: &Vault, mut id: Option<Uuid>) -> String {
    let mut parts = vec![];
    while let Some(fid) = id {
        match v.folders.iter().find(|f| f.id == fid) {
            Some(f) => {
                parts.insert(0, f.name.clone());
                id = f.parent;
            }
            None => break,
        }
    }
    parts.join("/")
}

fn first(i: &Item, k: FieldKind) -> &str {
    i.first_of(k).map(|f| f.value.as_str()).unwrap_or("")
}

fn live(v: &Vault) -> impl Iterator<Item = &Item> {
    v.items.iter().filter(|i| i.trashed_at.is_none())
}

pub fn bitwarden_json(v: &Vault) -> String {
    let mut folder_ids: Vec<(String, String)> = vec![];
    let mut items: Vec<Value> = vec![];
    for i in live(v) {
        let path = folder_path(v, i.folder_id);
        let folder_id = if path.is_empty() {
            Value::Null
        } else {
            let id = match folder_ids.iter().find(|(p, _)| p == &path) {
                Some((_, id)) => id.clone(),
                None => {
                    let id = Uuid::new_v4().to_string();
                    folder_ids.push((path.clone(), id.clone()));
                    id
                }
            };
            Value::String(id)
        };
        let username = i
            .first_of(FieldKind::Username)
            .or_else(|| i.first_of(FieldKind::Email))
            .map(|f| f.value.as_str())
            .unwrap_or("");
        let used: Vec<Uuid> = [
            i.first_of(FieldKind::Username).or_else(|| i.first_of(FieldKind::Email)),
            i.first_of(FieldKind::Password),
            i.first_of(FieldKind::Totp),
        ]
        .into_iter()
        .flatten()
        .map(|f| f.id)
        .collect();
        let uris: Vec<Value> = i
            .fields
            .iter()
            .filter(|f| f.kind == FieldKind::Url && !f.value.is_empty())
            .map(|f| json!({"match": null, "uri": f.value}))
            .collect();
        let fields: Vec<Value> = i
            .fields
            .iter()
            .filter(|f| !used.contains(&f.id) && f.kind != FieldKind::Url && !f.value.is_empty())
            .map(|f| json!({"name": f.name, "value": f.value, "type": if f.concealed { 1 } else { 0 }}))
            .collect();
        let is_login = i.template != "note";
        items.push(json!({
            "id": i.id.to_string(),
            "folderId": folder_id,
            "type": if is_login { 1 } else { 2 },
            "name": i.title,
            "notes": if i.description.is_empty() { Value::Null } else { Value::String(i.description.clone()) },
            "favorite": i.favorite,
            "fields": fields,
            "login": if is_login { json!({"uris": uris, "username": username, "password": first(i, FieldKind::Password), "totp": first(i, FieldKind::Totp)}) } else { Value::Null },
            "secureNote": if is_login { Value::Null } else { json!({"type": 0}) },
        }));
    }
    let folders: Vec<Value> = folder_ids.iter().map(|(p, id)| json!({"id": id, "name": p})).collect();
    serde_json::to_string_pretty(&json!({"encrypted": false, "folders": folders, "items": items})).unwrap_or_default()
}

pub fn csv(v: &Vault) -> String {
    let mut w = csv::Writer::from_writer(vec![]);
    let _ = w.write_record(["name", "url", "username", "password", "note", "totp", "folder"]);
    for i in live(v) {
        let user = i
            .first_of(FieldKind::Username)
            .or_else(|| i.first_of(FieldKind::Email))
            .map(|f| f.value.as_str())
            .unwrap_or("");
        let _ = w.write_record([
            i.title.as_str(),
            first(i, FieldKind::Url),
            user,
            first(i, FieldKind::Password),
            i.description.as_str(),
            first(i, FieldKind::Totp),
            &folder_path(v, i.folder_id),
        ]);
    }
    String::from_utf8(w.into_inner().unwrap_or_default()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ImportBatch, ImportedItem, merge};

    fn sample() -> Vault {
        let mut v = Vault::new("t");
        let mut a = ImportedItem::login("Example");
        a.push("Username", FieldKind::Username, "me");
        a.push("Password", FieldKind::Password, "p\"w,1");
        a.push("Website", FieldKind::Url, "https://example.com");
        a.push("Port", FieldKind::Number, "5432");
        a.folder_path = vec!["Work".into(), "DB".into()];
        merge(
            &mut v,
            ImportBatch {
                items: vec![a],
                ..Default::default()
            },
            false,
            None,
        );
        v
    }

    #[test]
    fn bitwarden_roundtrip() {
        let v = sample();
        let json = bitwarden_json(&v);
        let b = crate::bitwarden::parse(json.as_bytes()).unwrap();
        assert_eq!(b.items.len(), 1);
        let it = &b.items[0];
        assert_eq!(it.folder_path, vec!["Work", "DB"]);
        assert!(
            it.fields
                .iter()
                .any(|f| f.kind == FieldKind::Password && f.value == "p\"w,1")
        );
        assert!(it.fields.iter().any(|f| f.name == "Port" && f.value == "5432"));
    }

    #[test]
    fn csv_roundtrip() {
        let v = sample();
        let text = csv(&v);
        let b = crate::csv_import::parse(text.as_bytes()).unwrap();
        assert_eq!(b.items[0].title, "Example");
        assert!(
            b.items[0]
                .fields
                .iter()
                .any(|f| f.kind == FieldKind::Password && f.value == "p\"w,1")
        );
        assert_eq!(b.items[0].folder_path, vec!["Work", "DB"]);
    }
}
