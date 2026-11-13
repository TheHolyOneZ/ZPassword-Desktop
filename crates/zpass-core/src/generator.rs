use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::crypto::random_below;
use crate::error::{Error, Result};

static WORDLIST: &str = include_str!("../data/eff_large_wordlist.txt");

fn words() -> impl Iterator<Item = &'static str> {
    WORDLIST.lines().filter(|l| !l.is_empty())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GeneratorMode {
    Characters,
    Passphrase,
    Pin,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GeneratorOptions {
    pub mode: GeneratorMode,
    pub length: u32,
    pub uppercase: bool,
    pub lowercase: bool,
    pub digits: bool,
    pub symbols: bool,
    pub avoid_ambiguous: bool,
    pub words: u32,
    pub separator: String,
    pub capitalize: bool,
    pub append_number: bool,
}

impl Default for GeneratorOptions {
    fn default() -> Self {
        Self {
            mode: GeneratorMode::Characters,
            length: 24,
            uppercase: true,
            lowercase: true,
            digits: true,
            symbols: true,
            avoid_ambiguous: false,
            words: 5,
            separator: "-".into(),
            capitalize: false,
            append_number: false,
        }
    }
}

const UPPER: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const LOWER: &str = "abcdefghijklmnopqrstuvwxyz";
const DIGITS: &str = "0123456789";
const SYMBOLS: &str = "!@#$%^&*()-_=+[]{};:,.<>/?~";
const AMBIGUOUS: &str = "Il1O0o|`'\"";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Generated {
    pub value: String,
    pub entropy_bits: f64,
}

impl Drop for Generated {
    fn drop(&mut self) {
        zeroize::Zeroize::zeroize(&mut self.value);
    }
}

fn pick<T: Copy>(set: &[T]) -> Result<T> {
    Ok(set[random_below(set.len() as u32)? as usize])
}

pub fn generate(o: &GeneratorOptions) -> Result<Generated> {
    match o.mode {
        GeneratorMode::Characters => characters(o),
        GeneratorMode::Passphrase => passphrase(o),
        GeneratorMode::Pin => {
            let len = o.length.clamp(4, 32);
            let digits: Vec<char> = DIGITS.chars().collect();
            let mut v = String::with_capacity(len as usize);
            for _ in 0..len {
                v.push(pick(&digits)?);
            }
            Ok(Generated {
                value: v,
                entropy_bits: len as f64 * 10f64.log2(),
            })
        }
    }
}

fn characters(o: &GeneratorOptions) -> Result<Generated> {
    let len = o.length.clamp(4, 256) as usize;
    let filter = |s: &str| -> Vec<char> {
        s.chars()
            .filter(|c| !(o.avoid_ambiguous && AMBIGUOUS.contains(*c)))
            .collect()
    };
    let mut classes: Vec<Vec<char>> = Vec::new();
    if o.uppercase {
        classes.push(filter(UPPER));
    }
    if o.lowercase {
        classes.push(filter(LOWER));
    }
    if o.digits {
        classes.push(filter(DIGITS));
    }
    if o.symbols {
        classes.push(filter(SYMBOLS));
    }
    if classes.is_empty() {
        return Err(Error::Invalid("Select at least one character set.".into()));
    }
    let pool: Vec<char> = classes.iter().flatten().copied().collect();
    let mut out: Zeroizing<Vec<char>> = Zeroizing::new(Vec::with_capacity(len));
    for c in &classes {
        out.push(pick(c)?);
    }
    while out.len() < len {
        out.push(pick(&pool)?);
    }
    for i in (1..out.len()).rev() {
        let j = random_below(i as u32 + 1)? as usize;
        out.swap(i, j);
    }
    let entropy_bits = len as f64 * (pool.len() as f64).log2();
    Ok(Generated {
        value: out.iter().collect(),
        entropy_bits,
    })
}

fn passphrase(o: &GeneratorOptions) -> Result<Generated> {
    let n = o.words.clamp(3, 20);
    let list: Vec<&str> = words().collect();
    let mut parts: Vec<String> = Vec::with_capacity(n as usize);
    for _ in 0..n {
        let w = pick(&list)?;
        parts.push(if o.capitalize {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                .unwrap_or_default()
        } else {
            w.to_string()
        });
    }
    let mut entropy = n as f64 * (list.len() as f64).log2();
    if o.append_number {
        let idx = random_below(n)? as usize;
        parts[idx].push(char::from(b'0' + random_below(10)? as u8));
        entropy += 10f64.log2();
    }
    let sep: String = o.separator.chars().take(3).collect();
    let value = parts.join(&sep);
    parts.iter_mut().for_each(zeroize::Zeroize::zeroize);
    Ok(Generated {
        value,
        entropy_bits: entropy,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wordlist_loaded() {
        assert_eq!(words().count(), 7776);
    }

    #[test]
    fn characters_respect_classes() {
        let o = GeneratorOptions {
            length: 40,
            symbols: false,
            ..Default::default()
        };
        for _ in 0..200 {
            let g = generate(&o).unwrap();
            assert_eq!(g.value.chars().count(), 40);
            assert!(g.value.chars().all(|c| c.is_ascii_alphanumeric()));
            assert!(g.value.chars().any(|c| c.is_ascii_uppercase()));
            assert!(g.value.chars().any(|c| c.is_ascii_lowercase()));
            assert!(g.value.chars().any(|c| c.is_ascii_digit()));
        }
    }

    #[test]
    fn ambiguous_excluded() {
        let o = GeneratorOptions {
            length: 200,
            avoid_ambiguous: true,
            ..Default::default()
        };
        let g = generate(&o).unwrap();
        assert!(!g.value.chars().any(|c| AMBIGUOUS.contains(c)));
    }

    #[test]
    fn passphrase_shape() {
        let o = GeneratorOptions {
            mode: GeneratorMode::Passphrase,
            words: 6,
            separator: ".".into(),
            ..Default::default()
        };
        let g = generate(&o).unwrap();
        assert_eq!(g.value.split('.').count(), 6);
        assert!((g.entropy_bits - 6.0 * 7776f64.log2()).abs() < 1e-9);
    }

    #[test]
    fn empty_classes_rejected() {
        let o = GeneratorOptions {
            uppercase: false,
            lowercase: false,
            digits: false,
            symbols: false,
            ..Default::default()
        };
        assert!(generate(&o).is_err());
    }
}
