use serde::Serialize;
use reqwest::header::HeaderMap;
use crate::db::models::{HeaderMapEntry, RequestMetadata, ResponseMetadata};

pub trait ToMetadata {
    type Output: Serialize;
    fn to_metadata(&self) -> Self::Output;
}

impl ToMetadata for HeaderMap {
    type Output = Vec<HeaderMapEntry>;
    fn to_metadata(&self) -> Self::Output {
        self.iter()
            .map(|(k, v)| {
                let k = k.to_string();
                match v.to_str() {
                    Ok(v) => HeaderMapEntry::new(k, v.to_string()),
                    _ => HeaderMapEntry::new(k + ":b64", base64::encode(v.as_bytes())),
                }
            })
            .collect()
    }
}

impl ToMetadata for reqwest::Request {
    type Output = RequestMetadata;
    fn to_metadata(&self) -> Self::Output {
        RequestMetadata {
            headers: self.headers().clone().to_metadata(),
            method: self.method().to_string(),
            url: self.url().to_string(),
        }
    }
}

impl ToMetadata for reqwest::Response {
    type Output = ResponseMetadata;
    fn to_metadata(&self) -> Self::Output {
        ResponseMetadata {
            headers: self.headers().clone().to_metadata(),
            url: self.url().to_string(),
            status: self.status().as_u16()
        }
    }
}
