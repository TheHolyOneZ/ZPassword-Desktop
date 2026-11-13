use std::time::{SystemTime, UNIX_EPOCH};

pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub mod b64 {
    use data_encoding::BASE64;
    use serde::{Deserialize, Deserializer, Serializer, de::Error};

    pub fn serialize<S: Serializer, T: AsRef<[u8]>>(v: &T, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&BASE64.encode(v.as_ref()))
    }

    pub fn deserialize<'de, D: Deserializer<'de>, T: TryFrom<Vec<u8>>>(d: D) -> Result<T, D::Error> {
        let s = String::deserialize(d)?;
        let bytes = BASE64.decode(s.as_bytes()).map_err(D::Error::custom)?;
        T::try_from(bytes).map_err(|_| D::Error::custom("unexpected byte length"))
    }
}
