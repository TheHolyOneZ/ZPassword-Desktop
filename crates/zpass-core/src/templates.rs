use crate::model::{FieldKind::*, Template, TemplateField};

fn t(id: &str, name: &str, icon: &str, fields: &[(&str, crate::model::FieldKind)]) -> Template {
    Template {
        id: id.into(),
        name: name.into(),
        icon: icon.into(),
        fields: fields
            .iter()
            .map(|(n, k)| TemplateField {
                name: (*n).into(),
                kind: *k,
            })
            .collect(),
        builtin: true,
    }
}

pub fn builtin() -> Vec<Template> {
    vec![
        t(
            "login",
            "Login",
            "key",
            &[
                ("Username", Username),
                ("Email", Email),
                ("Password", Password),
                ("Website", Url),
                ("One-time code", Totp),
            ],
        ),
        t(
            "database",
            "Database",
            "database",
            &[
                ("Host", Text),
                ("Port", Number),
                ("Database", Text),
                ("Username", Username),
                ("Password", Password),
                ("Connection URL", Url),
            ],
        ),
        t(
            "server",
            "Server / SSH",
            "server",
            &[
                ("Host", Text),
                ("Port", Number),
                ("Username", Username),
                ("Password", Password),
                ("Private key", Hidden),
            ],
        ),
        t(
            "api",
            "API key",
            "code",
            &[("Key", Hidden), ("Secret", Password), ("Endpoint", Url)],
        ),
        t(
            "wifi",
            "Wi-Fi",
            "wifi",
            &[("Network name", Text), ("Password", Password), ("Security", Text)],
        ),
        t(
            "card",
            "Payment card",
            "card",
            &[
                ("Cardholder", Text),
                ("Number", Hidden),
                ("Expiry", Text),
                ("CVC", Password),
                ("PIN", Password),
            ],
        ),
        t(
            "identity",
            "Identity",
            "id",
            &[
                ("Full name", Text),
                ("Email", Email),
                ("Phone", Phone),
                ("Address", Multiline),
                ("Birthday", Date),
            ],
        ),
        t("note", "Secure note", "note", &[]),
    ]
}

pub fn find(user: &[Template], id: &str) -> Option<Template> {
    builtin()
        .into_iter()
        .find(|t| t.id == id)
        .or_else(|| user.iter().find(|t| t.id == id).cloned())
}
