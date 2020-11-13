use serde_with::{DeserializeAs, SerializeAs};
use serde_with::formats::{Format, Lowercase, Uppercase};
use serde::{Deserializer, Deserialize, Serializer};
use std::borrow::Cow;
use serde::de::Error;
use std::marker::PhantomData;

#[derive(Copy, Clone, Debug, Default)]
pub struct Base64 { }

impl<T> SerializeAs<T> for Base64
    where
        T: AsRef<[u8]>,
{
    fn serialize_as<S>(source: &T, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
    {
        serializer.serialize_str(&base64::encode(source))
    }
}

impl<'de, T> DeserializeAs<'de, T> for Base64
    where
        T: From<Vec<u8>>
{
    fn deserialize_as<D>(deserializer: D) -> Result<T, D::Error>
        where
            D: Deserializer<'de>,
    {
        <Cow<'de, str> as Deserialize<'de>>::deserialize(deserializer)
            .and_then(|s| base64::decode(&*s).map_err(Error::custom))
            .map(Into::into)
    }
}
