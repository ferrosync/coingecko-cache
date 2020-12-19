use serde::Serialize;
use serde_with::SerializeAs;
use serde_json::value::RawValue;

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
