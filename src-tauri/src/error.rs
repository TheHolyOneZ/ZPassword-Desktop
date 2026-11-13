use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiError {
    pub code: &'static str,
    pub message: String,
}

impl ApiError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new("invalid", message)
    }
    pub fn state(message: impl Into<String>) -> Self {
        Self::new("wrongState", message)
    }
}

impl From<zpass_core::Error> for ApiError {
    fn from(e: zpass_core::Error) -> Self {
        use zpass_core::Error as E;
        let code = match &e {
            E::InvalidCredentials => "invalidCredentials",
            E::Corrupt(_) | E::UnsupportedVersion(_) => "corrupt",
            E::Locked => "inUse",
            E::Invalid(_) | E::KdfParams => "invalid",
            E::NotFound(_) => "notFound",
            E::Io(_) => "io",
            _ => "internal",
        };
        let message = match &e {
            E::InvalidCredentials => "Those credentials did not open the vault.".to_string(),
            other => other.to_string(),
        };
        Self { code, message }
    }
}

impl From<std::io::Error> for ApiError {
    fn from(e: std::io::Error) -> Self {
        Self::new("io", e.to_string())
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
