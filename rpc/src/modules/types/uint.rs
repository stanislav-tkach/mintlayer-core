use bigint::U256 as GlobalU256;
use byteorder::{BigEndian, ByteOrder, LittleEndian};
use rustc_hex::FromHexError;
use serde;
use serde::de::Unexpected;
use std::fmt;
use std::str::FromStr;

#[inline]
pub fn bits(val: &GlobalU256) -> usize {
    let &GlobalU256(ref arr) = val;
    for i in 1..4 {
        if (*arr)[4 - i] > 0 {
            return (0x40 * (4 - i + 1)) - arr[4 - i].leading_zeros() as usize;
        }
    }
    0x40 - arr[0].leading_zeros() as usize
}

#[inline]
pub fn is_zero(val: &GlobalU256) -> bool {
    let &GlobalU256(ref arr) = &val;
    for i in 0..4 {
        if arr[i] != 0 {
            return false;
        }
    }
    return true;
}

#[inline]
pub fn to_big_endian(val: &GlobalU256, bytes: &mut [u8]) {
    debug_assert!(4 * 8 == bytes.len());
    let &GlobalU256(ref arr) = val;
    for i in 0..4 {
        BigEndian::write_u64(&mut bytes[8 * i..], arr[4 - i - 1]);
    }
}

#[inline]
pub fn to_hex(val: &GlobalU256) -> String {
    use core::cmp;
    use rustc_hex::ToHex;

    if is_zero(val) {
        return "0".to_owned();
    } // special case.
    let mut bytes = [0u8; 8 * 4];
    to_big_endian(val, &mut bytes);
    let bp7 = bits(val) + 7;
    let len = cmp::max(bp7 / 8, 1);
    let bytes_hex = bytes[bytes.len() - len..].to_hex::<String>();
    (&bytes_hex[1 - bp7 % 8 / 4..]).to_owned()
}

fn from_str(value: &str) -> Result<GlobalU256, FromHexError> {
    use rustc_hex::FromHex;

    let bytes: Vec<u8> = match value.len() % 2 == 0 {
        true => value.from_hex()?,
        false => ("0".to_owned() + value).from_hex()?,
    };

    let bytes_ref: &[u8] = &bytes;
    Ok(From::from(bytes_ref))
}

macro_rules! impl_uint {
    ($name: ident, $other: ident, $size: expr) => {
        /// Uint serialization.
        #[derive(Debug, Default, Clone, Copy, PartialEq, Hash)]
        pub struct $name($other);

        impl Eq for $name {}

        impl<T> From<T> for $name
        where
            $other: From<T>,
        {
            fn from(o: T) -> Self {
                $name($other::from(o))
            }
        }
        /*
                impl FromStr for $name {
                    type Err = <$other as FromStr>::Err;

                    fn from_str(s: &str) -> Result<Self, Self::Err> {
                        $other::from_str(s).map($name)
                    }
                }
        */
        impl Into<$other> for $name {
            fn into(self) -> $other {
                self.0
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                let as_hex = format!("{}", to_hex(&self.0));
                serializer.serialize_str(&as_hex)
            }
        }

        impl<'a> serde::Deserialize<'a> for $name {
            fn deserialize<D>(deserializer: D) -> Result<$name, D::Error>
            where
                D: serde::Deserializer<'a>,
            {
                struct UintVisitor;

                impl<'b> serde::de::Visitor<'b> for UintVisitor {
                    type Value = $name;

                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        formatter.write_str("an integer represented in hex string")
                    }

                    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
                    where
                        E: serde::de::Error,
                    {
                        if value.len() > $size * 16 {
                            return Err(E::invalid_value(Unexpected::Str(value), &self));
                        }

                        from_str(value)
                            .map($name)
                            .map_err(|_| E::invalid_value(Unexpected::Str(value), &self))
                    }

                    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
                    where
                        E: serde::de::Error,
                    {
                        self.visit_str(&value)
                    }
                }

                deserializer.deserialize_identifier(UintVisitor)
            }
        }
    };
}

impl_uint!(U256, GlobalU256, 4);

#[cfg(test)]
mod tests {
    use super::U256;
    use serde_json;

    #[test]
    fn u256_serialize() {
        let u256 = U256::from(256);
        let serialized = serde_json::to_string(&u256).unwrap();
        assert_eq!(serialized, r#""100""#);
    }

    #[test]
    fn u256_deserialize() {
        let u256 = U256::from(256);
        let deserialized = serde_json::from_str::<U256>(r#""100""#).unwrap();
        assert_eq!(deserialized, u256);
    }
}
