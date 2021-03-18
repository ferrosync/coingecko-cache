use std::fmt;
use std::marker::PhantomData;
use std::str::FromStr;
use std::fmt::Display;

use serde::{Serialize, Deserializer, Deserialize};
use serde::de::{self, Visitor, MapAccess};
use serde_with::{SerializeAs};
use serde_json::value::RawValue;

#[cfg(feature = "serde_deser_unquoted_bigdecimal")]
mod big_decimal_exact {
    use serde_with::DeserializeAs;
    use bigdecimal::BigDecimal;
    use serde::{Deserializer, Deserialize};
    use std::str::FromStr;

    pub struct BigDecimalExact;

    impl<'de> DeserializeAs<'de, BigDecimal> for BigDecimalExact
    {
        fn deserialize_as<D>(deserializer: D) -> Result<BigDecimal, D::Error>
            where
                D: Deserializer<'de>,
        {
            <serde_json::Number as Deserialize<'de>>::deserialize(deserializer)
                .and_then(|s| BigDecimal::from_str(&*s.to_string()).map_err(serde::de::Error::custom))
                .map(Into::into)
        }
    }
}

#[cfg(feature = "serde_deser_unquoted_bigdecimal")]
pub use big_decimal_exact::*;

pub struct ToStringVerbatim { }

impl<T> SerializeAs<T> for ToStringVerbatim
    where
        T: ToString,
{
    fn serialize_as<S>(source: &T, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
    {
        let raw_value = RawValue::from_string(source.to_string()).unwrap(); // HACK!
        raw_value.serialize(serializer)
    }
}

// source: https://serde.rs/string-or-struct.html
//
pub fn string_or_struct<'de, T, D, R>(deserializer: D) -> Result<T, D::Error>
    where
        T: Deserialize<'de> + FromStr<Err = R>,
        D: Deserializer<'de>,
        R: Display,
{
    // This is a Visitor that forwards string types to T's `FromStr` impl and
    // forwards map types to T's `Deserialize` impl. The `PhantomData` is to
    // keep the compiler from complaining about T being an unused generic type
    // parameter. We need T in order to know the Value type for the Visitor
    // impl.
    struct StringOrStruct<T, RR>(PhantomData<fn() -> T>, PhantomData<fn() -> RR>);

    impl<'de, T, RR> Visitor<'de> for StringOrStruct<T, RR>
        where
            T: Deserialize<'de> + FromStr<Err = RR>,
            RR: Display
    {
        type Value = T;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("string or map")
        }

        fn visit_str<E>(self, value: &str) -> Result<T, E>
            where
                E: de::Error,
        {
            FromStr::from_str(value).map_err(de::Error::custom)
        }

        fn visit_map<M>(self, map: M) -> Result<T, M::Error>
            where
                M: MapAccess<'de>,
        {
            // `MapAccessDeserializer` is a wrapper that turns a `MapAccess`
            // into a `Deserializer`, allowing it to be used as the input to T's
            // `Deserialize` implementation. T then deserializes itself using
            // the entries from the map visitor.
            Deserialize::deserialize(de::value::MapAccessDeserializer::new(map))
        }
    }

    deserializer.deserialize_any(StringOrStruct(PhantomData, PhantomData))
}
