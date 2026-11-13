use std::collections::HashMap;

use serde::Serialize;
use uuid::Uuid;

use crate::crypto::sha256;
use crate::model::{FieldKind, Vault};
use crate::util::now;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Strength {
    pub score: u8,
    pub guesses_log10: f64,
    pub crack_time: String,
    pub warning: Option<String>,
    pub suggestions: Vec<String>,
}

pub fn strength(password: &str, user_inputs: &[&str]) -> Strength {
    if password.is_empty() {
        return Strength {
            score: 0,
            guesses_log10: 0.0,
            crack_time: "instantly".into(),
            warning: None,
            suggestions: vec![],
        };
    }
    let e = zxcvbn::zxcvbn(password, user_inputs);
    let feedback = e.feedback();
    Strength {
        score: u8::from(e.score()),
        guesses_log10: e.guesses_log10(),
        crack_time: e.crack_times().offline_slow_hashing_1e4_per_second().to_string(),
        warning: feedback.and_then(|f| f.warning()).map(|w| w.to_string()),
        suggestions: feedback
            .map(|f| f.suggestions().iter().map(|s| s.to_string()).collect())
            .unwrap_or_default(),
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthIssue {
    pub item_id: Uuid,
    pub field_id: Uuid,
    pub field_name: String,
    pub weak: bool,
    pub score: u8,
    pub reused_with: usize,
    pub age_days: i64,
    pub old: bool,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct HealthReport {
    pub total_passwords: usize,
    pub weak: usize,
    pub reused: usize,
    pub old: usize,
    pub score: u8,
    pub issues: Vec<HealthIssue>,
}

pub fn report(vault: &Vault) -> HealthReport {
    let max_age = vault.settings.password_max_age_days as i64;
    let t = now();
    let mut by_hash: HashMap<[u8; 32], Vec<Uuid>> = HashMap::new();
    let mut entries = Vec::new();
    for item in vault.items.iter().filter(|i| i.trashed_at.is_none()) {
        for f in item
            .fields
            .iter()
            .filter(|f| f.kind == FieldKind::Password && !f.value.is_empty())
        {
            let h = sha256(&[b"zpass/health", f.value.as_bytes()]);
            let ids = by_hash.entry(h).or_default();
            if !ids.contains(&item.id) {
                ids.push(item.id);
            }
            let s = strength(&f.value, &[&item.title]);
            let age_days = (t - item.field_changed_at(&f.id)).max(0) / 86_400;
            entries.push((
                h,
                HealthIssue {
                    item_id: item.id,
                    field_id: f.id,
                    field_name: f.name.clone(),
                    weak: s.score < 3,
                    score: s.score,
                    reused_with: 0,
                    age_days,
                    old: max_age > 0 && age_days > max_age,
                },
            ));
        }
    }
    let mut r = HealthReport {
        total_passwords: entries.len(),
        ..Default::default()
    };
    for (h, mut issue) in entries {
        issue.reused_with = by_hash.get(&h).map(|v| v.len().saturating_sub(1)).unwrap_or(0);
        r.weak += issue.weak as usize;
        r.reused += (issue.reused_with > 0) as usize;
        r.old += issue.old as usize;
        if issue.weak || issue.reused_with > 0 || issue.old {
            r.issues.push(issue);
        }
    }
    r.score = if r.total_passwords == 0 {
        100
    } else {
        let bad = r.weak as f64 * 1.0 + r.reused as f64 * 0.8 + r.old as f64 * 0.3;
        (100.0 * (1.0 - (bad / (r.total_passwords as f64 * 2.1)).min(1.0))).round() as u8
    };
    r.issues
        .sort_by_key(|i| (std::cmp::Reverse(i.weak), std::cmp::Reverse(i.reused_with), i.score));
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Field, Item};

    fn item(title: &str, pw: &str) -> Item {
        Item {
            id: Uuid::new_v4(),
            serial: 1,
            template: "login".into(),
            title: title.into(),
            description: String::new(),
            folder_id: None,
            labels: vec![],
            favorite: false,
            fields: vec![Field::new("Password", FieldKind::Password, pw)],
            history: vec![],
            created_at: now(),
            updated_at: now(),
            trashed_at: None,
        }
    }

    #[test]
    fn detects_weak_and_reused() {
        let mut v = Vault::new("t");
        v.items.push(item("a", "password"));
        v.items.push(item("b", "password"));
        v.items.push(item("c", "q8$Lr2!vZp@9nW#eT4xK"));
        let r = report(&v);
        assert_eq!(r.total_passwords, 3);
        assert_eq!(r.weak, 2);
        assert_eq!(r.reused, 2);
        assert!(r.score < 100);
    }

    #[test]
    fn strength_ordering() {
        assert!(strength("password", &[]).score <= 1);
        assert_eq!(strength("correct-horse-battery-staple-quantum-fig", &[]).score, 4);
    }
}
