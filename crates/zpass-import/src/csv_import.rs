use zpass_core::model::FieldKind;

use crate::{ImportBatch, ImportedItem, Result, Source, fmt_err};

fn find(headers: &[String], names: &[&str]) -> Option<usize> {
    names.iter().find_map(|n| headers.iter().position(|h| h == n))
}

fn detect(h: &[String]) -> Source {
    let has = |n: &str| h.iter().any(|x| x == n);
    if has("login_uri") && has("login_password") {
        Source::BitwardenCsv
    } else if has("httprealm") || has("formactionorigin") {
        Source::Firefox
    } else if has("grouping") && has("extra") {
        Source::LastPass
    } else if has("otpauth") && has("title") && has("archived") {
        Source::OnePassword
    } else if has("otpauth") && has("title") {
        Source::Safari
    } else if has("name") && has("url") && has("username") && has("password") && h.len() <= 5 {
        Source::Chrome
    } else {
        Source::GenericCsv
    }
}

pub fn parse(bytes: &[u8]) -> Result<ImportBatch> {
    let text = String::from_utf8_lossy(bytes);
    let text = text.trim_start_matches('\u{feff}');
    let mut rdr = csv::ReaderBuilder::new().flexible(true).from_reader(text.as_bytes());
    let headers: Vec<String> = rdr
        .headers()
        .map_err(|_| fmt_err("This file doesn't look like a CSV export."))?
        .iter()
        .map(|h| h.trim().to_lowercase().replace([' ', '-'], "_"))
        .collect();

    let title = find(&headers, &["name", "title", "account", "site", "service"]);
    let url = find(
        &headers,
        &[
            "login_uri",
            "url",
            "website",
            "web_site",
            "uri",
            "address",
            "origin_url",
            "hostname",
        ],
    );
    let user = find(
        &headers,
        &[
            "login_username",
            "username",
            "user",
            "login",
            "user_name",
            "email",
            "e_mail",
            "login_name",
        ],
    );
    let pass = find(&headers, &["login_password", "password", "pass", "passwd", "pwd"]);
    let notes = find(
        &headers,
        &["notes", "note", "extra", "comments", "comment", "description"],
    );
    let totp = find(
        &headers,
        &["login_totp", "totp", "otpauth", "otp", "one_time_password", "2fa"],
    );
    let folder = find(&headers, &["folder", "grouping", "group", "category", "vault"]);
    let fav = find(&headers, &["favorite", "fav", "favourite"]);
    let tags = find(&headers, &["tags", "labels"]);
    if pass.is_none() && user.is_none() {
        return Err(fmt_err(
            "Couldn't find username or password columns. The first row must name the columns.",
        ));
    }

    let source = detect(&headers);
    let mut batch = ImportBatch {
        source: Some(source),
        ..Default::default()
    };
    let mut bad_rows = 0usize;
    for (n, rec) in rdr.records().enumerate() {
        let Ok(rec) = rec else {
            bad_rows += 1;
            continue;
        };
        let get = |i: Option<usize>| i.and_then(|i| rec.get(i)).map(str::trim).unwrap_or("");
        if [get(title), get(url), get(user), get(pass)]
            .iter()
            .all(|v| v.is_empty())
        {
            continue;
        }
        let kind_of_type = headers
            .iter()
            .position(|h| h == "type")
            .and_then(|i| rec.get(i))
            .unwrap_or("login");
        let mut it = ImportedItem::login(get(title));
        if source == Source::BitwardenCsv && kind_of_type == "note" {
            it.template = "note".into();
        }
        let u = get(user);
        it.push(
            "Username",
            if u.contains('@') {
                FieldKind::Email
            } else {
                FieldKind::Username
            },
            u,
        );
        it.push("Password", FieldKind::Password, get(pass));
        it.push("Website", FieldKind::Url, get(url));
        it.push("One-time code", FieldKind::Totp, get(totp));
        it.description = get(notes).to_string();
        let f = get(folder);
        if !f.is_empty() {
            it.folder_path = f
                .split(['/', '\\'])
                .filter(|s| !s.trim().is_empty())
                .map(|s| s.trim().to_string())
                .collect();
        }
        it.favorite = matches!(get(fav).to_lowercase().as_str(), "1" | "true" | "yes");
        it.labels = get(tags)
            .split([',', ';'])
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();
        it.ensure_title();
        if it.fields.is_empty() && it.description.is_empty() {
            continue;
        }
        let _ = n;
        batch.items.push(it);
    }
    if bad_rows > 0 {
        batch.warnings.push(format!("{bad_rows} malformed rows were skipped."));
    }
    if batch.items.is_empty() {
        return Err(fmt_err("No entries found in this CSV."));
    }
    Ok(batch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chrome() {
        let b =
            parse(b"name,url,username,password,note\nExample,https://example.com/,me,pw1,hello\n,https://x.org,u,p,\n")
                .unwrap();
        assert_eq!(b.source, Some(Source::Chrome));
        assert_eq!(b.items.len(), 2);
        assert_eq!(b.items[1].title, "x.org");
        assert_eq!(b.items[0].description, "hello");
    }

    #[test]
    fn firefox_with_bom_and_quotes() {
        let csv = "\u{feff}\"url\",\"username\",\"password\",\"httpRealm\",\"formActionOrigin\",\"guid\",\"timeCreated\",\"timeLastUsed\",\"timePasswordChanged\"\n\"https://a.example\",\"me@a.example\",\"p,w\",,\"https://a.example\",\"{x}\",\"1\",\"1\",\"1\"\n";
        let b = parse(csv.as_bytes()).unwrap();
        assert_eq!(b.source, Some(Source::Firefox));
        let pw = b.items[0]
            .fields
            .iter()
            .find(|f| f.kind == FieldKind::Password)
            .unwrap();
        assert_eq!(pw.value, "p,w");
        assert!(b.items[0].fields.iter().any(|f| f.kind == FieldKind::Email));
    }

    #[test]
    fn bitwarden_csv_folders_and_totp() {
        let csv = "folder,favorite,type,name,notes,fields,reprompt,login_uri,login_username,login_password,login_totp\nWork/DB,1,login,PG,,,0,,admin,s3cret,JBSWY3DPEHPK3PXP\n";
        let b = parse(csv.as_bytes()).unwrap();
        assert_eq!(b.source, Some(Source::BitwardenCsv));
        assert_eq!(b.items[0].folder_path, vec!["Work", "DB"]);
        assert!(b.items[0].favorite);
        assert!(b.items[0].fields.iter().any(|f| f.kind == FieldKind::Totp));
    }

    #[test]
    fn rejects_unrelated_csv() {
        assert!(parse(b"a,b,c\n1,2,3\n").is_err());
    }
}
