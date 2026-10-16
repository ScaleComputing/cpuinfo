//! Serde helpers so definition files can write CPUID leaves and MSR addresses in hex
//!
//! JSON has no hex literals, so hex values are written as strings such as `"0x80000001"`.
//! Decimal numbers and strings are also accepted, and `_` may be used as a digit separator
//! (`"0x8000_0001"`). Values below 10 read the same in either base, so they are written in decimal.

use serde::de::{self, Deserialize, Deserializer};
use serde::ser::Serializer;
use std::collections::BTreeMap;
use std::num::ParseIntError;

/// Format a u32 the way definition files write it: decimal below 10, otherwise `0x` hex
pub fn format_u32(value: u32) -> String {
    if value < 10 {
        value.to_string()
    } else {
        format!("{:#x}", value)
    }
}

/// Parse a `0x`-prefixed hex or a decimal string into a u32
pub fn parse_u32(text: &str) -> Result<u32, ParseIntError> {
    let digits = text.replace('_', "");
    match digits
        .strip_prefix("0x")
        .or_else(|| digits.strip_prefix("0X"))
    {
        Some(hex) => u32::from_str_radix(hex, 16),
        None => digits.parse(),
    }
}

/// `#[serde(with = "hex::u32")]` for a u32 that may be written as a hex string or a number
pub mod u32 {
    use super::*;

    #[derive(serde::Deserialize)]
    #[serde(untagged)]
    enum NumOrString {
        Num(u32),
        String(String),
    }

    pub fn serialize<S: Serializer>(value: &u32, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&format_u32(*value))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u32, D::Error> {
        match NumOrString::deserialize(deserializer)? {
            NumOrString::Num(value) => Ok(value),
            NumOrString::String(text) => parse_u32(&text)
                .map_err(|e| de::Error::custom(format!("invalid number {:?}: {}", text, e))),
        }
    }
}

/// `#[serde(with = "hex::u32_map")]` for a `BTreeMap<u32, V>` whose keys may be hex strings
pub mod u32_map {
    use super::*;
    use serde::Serialize;

    pub fn serialize<S, V>(map: &BTreeMap<u32, V>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        V: Serialize,
    {
        serializer.collect_map(map.iter().map(|(key, value)| (format_u32(*key), value)))
    }

    pub fn deserialize<'de, D, V>(deserializer: D) -> Result<BTreeMap<u32, V>, D::Error>
    where
        D: Deserializer<'de>,
        V: Deserialize<'de>,
    {
        let raw: Vec<(String, V)> =
            deserializer.deserialize_map(EntriesVisitor(Default::default()))?;
        let mut map = BTreeMap::new();
        for (text, value) in raw {
            let key = parse_u32(&text)
                .map_err(|e| de::Error::custom(format!("invalid key {:?}: {}", text, e)))?;
            // "0x7" and "7" are the same leaf; refuse rather than silently keep one
            if map.insert(key, value).is_some() {
                return Err(de::Error::custom(format!("duplicate key {:#x}", key)));
            }
        }
        Ok(map)
    }

    /// Collect map entries in order without merging duplicate keys
    struct EntriesVisitor<V>(std::marker::PhantomData<V>);

    impl<'de, V: Deserialize<'de>> de::Visitor<'de> for EntriesVisitor<V> {
        type Value = Vec<(String, V)>;

        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(f, "a map keyed by hex or decimal numbers")
        }

        fn visit_map<A: de::MapAccess<'de>>(self, mut access: A) -> Result<Self::Value, A::Error> {
            let mut entries = Vec::new();
            while let Some(entry) = access.next_entry()? {
                entries.push(entry);
            }
            Ok(entries)
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Sample {
        #[serde(with = "u32_map")]
        map: BTreeMap<u32, String>,
        #[serde(with = "u32")]
        address: u32,
    }

    #[test]
    fn parse_u32_formats() {
        assert_eq!(parse_u32("0x10a"), Ok(0x10a));
        assert_eq!(parse_u32("0X10A"), Ok(0x10a));
        assert_eq!(parse_u32("0x8000_0001"), Ok(0x8000_0001));
        assert_eq!(parse_u32("266"), Ok(266));
        assert!(parse_u32("0x").is_err());
        assert!(parse_u32("0x1_0000_0000").is_err());
        assert!(parse_u32("ten").is_err());
    }

    #[test]
    fn hex_and_decimal_inputs() {
        let sample: Sample =
            serde_json::from_str(r#"{"map": {"0x80000001": "a", "7": "b"}, "address": "0x48d"}"#)
                .unwrap();
        assert_eq!(sample.map[&0x80000001], "a");
        assert_eq!(sample.map[&7], "b");
        assert_eq!(sample.address, 0x48d);

        let sample: Sample = serde_json::from_str(r#"{"map": {}, "address": 266}"#).unwrap();
        assert_eq!(sample.address, 0x10a);
    }

    #[test]
    fn rejects_bad_and_duplicate_keys() {
        assert!(serde_json::from_str::<Sample>(r#"{"map": {"0xzz": "a"}, "address": 0}"#).is_err());
        let err =
            serde_json::from_str::<Sample>(r#"{"map": {"0x7": "a", "7": "b"}, "address": 0}"#)
                .unwrap_err();
        assert!(err.to_string().contains("duplicate key 0x7"), "{}", err);
    }

    #[test]
    fn serializes_small_decimal_else_hex() {
        assert_eq!(format_u32(0), "0");
        assert_eq!(format_u32(9), "9");
        assert_eq!(format_u32(10), "0xa");
        assert_eq!(format_u32(0x80000001), "0x80000001");

        let sample = Sample {
            map: BTreeMap::from([(7, "b".to_string()), (0xd, "c".to_string())]),
            address: 0x10a,
        };
        let text = serde_json::to_string(&sample).unwrap();
        assert_eq!(text, r#"{"map":{"7":"b","0xd":"c"},"address":"0x10a"}"#);
        assert_eq!(serde_json::from_str::<Sample>(&text).unwrap(), sample);
    }
}
