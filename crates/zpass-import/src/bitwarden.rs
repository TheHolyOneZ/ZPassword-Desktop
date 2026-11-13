use serde::Deserialize;
use zpass_core::model::FieldKind;

use crate::{ImportBatch, ImportedItem, Result, Source, fmt_err};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Export {
    #[serde(default)]
    encrypted: bool,
    #[serde(default)]
    password_protected: bool,
    #[serde(default)]
    folders: Vec<BwFolder>,
    #[serde(default)]
    items: Vec<BwItem>,
}

#[derive(Deserialize)]
struct BwFolder {
    id: String,
    name: String,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct BwItem {
    #[serde(rename = "type")]
    kind: u8,
    name: String,
    notes: Option<String>,
    favorite: bool,
    folder_id: Option<String>,
    fields: Vec<BwField>,
    login: Option<BwLogin>,
    card: Option<BwCard>,
    identity: Option<BwIdentity>,
    ssh_key: Option<BwSsh>,
    creation_date: Option<String>,
    revision_date: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct BwField {
    name: Option<String>,
    value: Option<String>,
    #[serde(rename = "type")]
    kind: u8,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct BwLogin {
    uris: Option<Vec<BwUri>>,
    username: Option<String>,
    password: Option<String>,
    totp: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct BwUri {
    uri: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct BwCard {
    cardholder_name: Option<String>,
    number: Option<String>,
    exp_month: Option<String>,
    exp_year: Option<String>,
    code: Option<String>,
    brand: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct BwIdentity {
    first_name: Option<String>,
    middle_name: Option<String>,
    last_name: Option<String>,
    email: Option<String>,
    phone: Option<String>,
    address1: Option<String>,
    address2: Option<String>,
    city: Option<String>,
    state: Option<String>,
    postal_code: Option<String>,
    country: Option<String>,
    company: Option<String>,
    username: Option<String>,
    passport_number: Option<String>,
    license_number: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct BwSsh {
    private_key: Option<String>,
    public_key: Option<String>,
    key_fingerprint: Option<String>,
}

fn s(o: &Option<String>) -> &str {
    o.as_deref().unwrap_or("")
}

pub(crate) fn iso_to_unix(v: &str) -> Option<i64> {
    let (date, time) = v.split_once('T')?;
    let mut d = date.split('-').map(|x| x.parse::<i64>().ok());
    let (y, m, day) = (d.next()??, d.next()??, d.next()??);
    let mut t = time.trim_end_matches('Z').split(':');
    let (hh, mm) = (t.next()?.parse::<i64>().ok()?, t.next()?.parse::<i64>().ok()?);
    let ss = t
        .next()
        .and_then(|x| x.split('.').next()?.parse::<i64>().ok())
        .unwrap_or(0);
    let y2 = if m <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    Some(days * 86400 + hh * 3600 + mm * 60 + ss)
}

pub fn parse(bytes: &[u8]) -> Result<ImportBatch> {
    let export: Export = serde_json::from_slice(bytes).map_err(|_| fmt_err("This is not a Bitwarden JSON export."))?;
    if export.encrypted || export.password_protected {
        return Err(fmt_err(
            "This Bitwarden export is encrypted. In Bitwarden, export again as \"JSON\" (not \"JSON (Encrypted)\"), import it, then delete the file.",
        ));
    }
    let mut batch = ImportBatch {
        source: Some(Source::Bitwarden),
        ..Default::default()
    };
    for it in export.items {
        let folder_path = it
            .folder_id
            .as_ref()
            .and_then(|id| export.folders.iter().find(|f| &f.id == id))
            .map(|f| f.name.split('/').map(str::to_string).collect())
            .unwrap_or_default();
        let mut out = ImportedItem {
            title: it.name.trim().to_string(),
            description: s(&it.notes).to_string(),
            favorite: it.favorite,
            folder_path,
            created_at: it.creation_date.as_deref().and_then(iso_to_unix),
            updated_at: it.revision_date.as_deref().and_then(iso_to_unix),
            ..Default::default()
        };
        match it.kind {
            1 => {
                out.template = "login".into();
                let l = it.login.unwrap_or_default();
                out.push(
                    "Username",
                    if s(&l.username).contains('@') {
                        FieldKind::Email
                    } else {
                        FieldKind::Username
                    },
                    s(&l.username),
                );
                out.push("Password", FieldKind::Password, s(&l.password));
                for (i, u) in l.uris.unwrap_or_default().iter().enumerate() {
                    out.push(
                        if i == 0 { "Website" } else { "Website (alt)" },
                        FieldKind::Url,
                        s(&u.uri),
                    );
                }
                out.push("One-time code", FieldKind::Totp, s(&l.totp));
            }
            3 => {
                out.template = "card".into();
                let c = it.card.unwrap_or_default();
                out.push("Cardholder", FieldKind::Text, s(&c.cardholder_name));
                out.push("Brand", FieldKind::Text, s(&c.brand));
                out.push("Number", FieldKind::Hidden, s(&c.number));
                let exp = match (s(&c.exp_month), s(&c.exp_year)) {
                    ("", "") => String::new(),
                    (m, y) => format!("{m:0>2}/{y}"),
                };
                out.push("Expiry", FieldKind::Text, &exp);
                out.push("CVC", FieldKind::Password, s(&c.code));
            }
            4 => {
                out.template = "identity".into();
                let i = it.identity.unwrap_or_default();
                let name = [s(&i.first_name), s(&i.middle_name), s(&i.last_name)]
                    .iter()
                    .filter(|x| !x.is_empty())
                    .copied()
                    .collect::<Vec<_>>()
                    .join(" ");
                out.push("Full name", FieldKind::Text, &name);
                out.push("Email", FieldKind::Email, s(&i.email));
                out.push("Phone", FieldKind::Phone, s(&i.phone));
                let addr = [
                    s(&i.address1),
                    s(&i.address2),
                    format!("{} {}", s(&i.postal_code), s(&i.city)).trim(),
                    s(&i.state),
                    s(&i.country),
                ]
                .iter()
                .filter(|x| !x.is_empty())
                .copied()
                .collect::<Vec<_>>()
                .join("\n");
                out.push("Address", FieldKind::Multiline, &addr);
                out.push("Company", FieldKind::Text, s(&i.company));
                out.push("Username", FieldKind::Username, s(&i.username));
                out.push("Passport", FieldKind::Hidden, s(&i.passport_number));
                out.push("Licence", FieldKind::Hidden, s(&i.license_number));
            }
            5 => {
                out.template = "server".into();
                let k = it.ssh_key.unwrap_or_default();
                out.push("Private key", FieldKind::Hidden, s(&k.private_key));
                out.push("Public key", FieldKind::Multiline, s(&k.public_key));
                out.push("Fingerprint", FieldKind::Text, s(&k.key_fingerprint));
            }
            _ => out.template = "note".into(),
        }
        for f in &it.fields {
            let kind = match f.kind {
                1 => FieldKind::Hidden,
                _ => FieldKind::Text,
            };
            out.push(f.name.as_deref().unwrap_or("Field"), kind, s(&f.value));
        }
        out.ensure_title();
        batch.items.push(out);
    }
    Ok(batch)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
      "encrypted": false,
      "folders": [{"id": "f1", "name": "Work/Databases"}],
      "items": [
        {"type": 1, "name": "Example", "favorite": true, "folderId": "f1", "notes": "n",
         "login": {"username": "me@example.com", "password": "pw", "totp": "JBSWY3DPEHPK3PXP",
                   "uris": [{"uri": "https://example.com"}]},
         "fields": [{"name": "PIN", "value": "1234", "type": 1}],
         "creationDate": "2021-01-02T03:04:05.000Z"},
        {"type": 2, "name": "Note", "notes": "secret text", "secureNote": {"type": 0}},
        {"type": 3, "name": "Visa", "card": {"cardholderName": "A", "number": "4111", "expMonth": "9", "expYear": "2030", "code": "123"}}
      ]
    }"#;

    #[test]
    fn parses_items() {
        let b = parse(SAMPLE.as_bytes()).unwrap();
        assert_eq!(b.items.len(), 3);
        let l = &b.items[0];
        assert_eq!(l.folder_path, vec!["Work", "Databases"]);
        assert!(l.favorite);
        assert!(
            l.fields
                .iter()
                .any(|f| f.kind == FieldKind::Email && f.value == "me@example.com")
        );
        assert!(l.fields.iter().any(|f| f.kind == FieldKind::Totp));
        assert!(l.fields.iter().any(|f| f.name == "PIN" && f.concealed));
        assert_eq!(l.created_at, Some(1609556645));
        assert_eq!(b.items[1].template, "note");
        assert!(
            b.items[2]
                .fields
                .iter()
                .any(|f| f.name == "Expiry" && f.value == "09/2030")
        );
    }

    #[test]
    fn rejects_encrypted() {
        assert!(parse(br#"{"encrypted": true, "items": []}"#).is_err());
        assert!(parse(b"not json").is_err());
    }
}
