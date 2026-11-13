use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("the operating system random number generator failed")]
    Rng,
    #[error("invalid credentials")]
    InvalidCredentials,
    #[error("the vault file is corrupted or was tampered with: {0}")]
    Corrupt(&'static str),
    #[error("unsupported vault format version {0}")]
    UnsupportedVersion(u16),
    #[error("key derivation parameters are out of bounds")]
    KdfParams,
    #[error("key derivation failed")]
    Kdf,
    #[error("encryption failed")]
    Encrypt,
    #[error("the vault is already open in another ZPassword window")]
    Locked,
    #[error("{0}")]
    Invalid(String),
    #[error("not found: {0}")]
    NotFound(&'static str),
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
