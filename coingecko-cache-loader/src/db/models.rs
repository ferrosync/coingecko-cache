use std::fmt::{Display, Formatter};
use std::fmt;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Eq, PartialEq, Debug)]
pub struct ProvenanceId {
    pub uuid: Uuid,
    pub object_id: i64,
}

impl Display for ProvenanceId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "[uuid = {}, obj #{}]", self.uuid, self.object_id)
    }
}


#[derive(Serialize, Deserialize)]
pub struct HeaderMapEntry {
    pub key: String,
    pub value: String
}

impl HeaderMapEntry {
    pub fn new(key: String, value: String) -> HeaderMapEntry {
        HeaderMapEntry { key, value }
    }
}

pub type HeaderMapSerializable = Vec<HeaderMapEntry>;

#[derive(Serialize)]
pub struct RequestMetadata {
    pub method: String,
    pub url: String,
    pub headers: HeaderMapSerializable,
}

#[derive(Serialize)]
pub struct ResponseMetadata {
    pub url: String,
    pub status: u16,
    pub headers: HeaderMapSerializable,
}
