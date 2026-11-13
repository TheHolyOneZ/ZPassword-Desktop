use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::crypto::{NONCE_LEN, TAG_LEN};
use crate::error::{Error, Result};
use crate::slots::Slot;

pub const MAGIC: &[u8; 8] = b"ZPASSVLT";
pub const FORMAT_VERSION: u16 = 1;
const PREFIX_LEN: usize = 14;
const MAX_HEADER_LEN: usize = 1024 * 1024;
pub const MAX_FILE_LEN: usize = 1024 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Header {
    pub vault_id: Uuid,
    pub created_at: i64,
    pub writer: String,
    pub slots: Vec<Slot>,
}

#[derive(Debug, Clone)]
pub struct Container {
    pub header: Header,
    pub aad: Vec<u8>,
    pub nonce: [u8; NONCE_LEN],
    pub ciphertext: Vec<u8>,
}

impl Container {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAX_FILE_LEN {
            return Err(Error::Corrupt("file too large"));
        }
        if bytes.len() < PREFIX_LEN || &bytes[..8] != MAGIC {
            return Err(Error::Corrupt("not a ZPassword vault"));
        }
        let version = u16::from_le_bytes([bytes[8], bytes[9]]);
        if version != FORMAT_VERSION {
            return Err(Error::UnsupportedVersion(version));
        }
        let header_len = u32::from_le_bytes(bytes[10..14].try_into().unwrap()) as usize;
        if header_len == 0 || header_len > MAX_HEADER_LEN {
            return Err(Error::Corrupt("header length"));
        }
        let header_end = PREFIX_LEN
            .checked_add(header_len)
            .ok_or(Error::Corrupt("header length"))?;
        if bytes.len() < header_end + NONCE_LEN + TAG_LEN {
            return Err(Error::Corrupt("truncated file"));
        }
        let header: Header =
            serde_json::from_slice(&bytes[PREFIX_LEN..header_end]).map_err(|_| Error::Corrupt("header"))?;
        if header.slots.is_empty() {
            return Err(Error::Corrupt("no key slots"));
        }
        for slot in &header.slots {
            if let Some(kdf) = &slot.kdf {
                kdf.validate()?;
            }
        }
        let nonce: [u8; NONCE_LEN] = bytes[header_end..header_end + NONCE_LEN].try_into().unwrap();
        Ok(Self {
            header,
            aad: bytes[..header_end].to_vec(),
            nonce,
            ciphertext: bytes[header_end + NONCE_LEN..].to_vec(),
        })
    }

    pub fn build_aad(header: &Header) -> Result<Vec<u8>> {
        let json = serde_json::to_vec(header)?;
        if json.len() > MAX_HEADER_LEN {
            return Err(Error::Invalid("header too large".into()));
        }
        let mut out = Vec::with_capacity(PREFIX_LEN + json.len());
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
        out.extend_from_slice(&(json.len() as u32).to_le_bytes());
        out.extend_from_slice(&json);
        Ok(out)
    }

    pub fn assemble(aad: &[u8], nonce: &[u8; NONCE_LEN], ciphertext: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(aad.len() + NONCE_LEN + ciphertext.len());
        out.extend_from_slice(aad);
        out.extend_from_slice(nonce);
        out.extend_from_slice(ciphertext);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_garbage_without_panicking() {
        for input in [
            &b""[..],
            b"ZPASSVLT",
            b"ZPASSVLT\x01\x00\xff\xff\xff\xff",
            b"nope nope nope nope",
        ] {
            assert!(Container::parse(input).is_err());
        }
        let mut bad_version = MAGIC.to_vec();
        bad_version.extend_from_slice(&9u16.to_le_bytes());
        bad_version.extend_from_slice(&[0; 64]);
        assert!(matches!(
            Container::parse(&bad_version),
            Err(Error::UnsupportedVersion(9))
        ));
    }
}
