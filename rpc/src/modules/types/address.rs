use crate::modules::primitives::address::Address;
use serde::de::{Unexpected, Visitor};
use serde::{Deserializer, Serialize, Serializer};
use std::fmt;

pub fn serialize<S>(address: &Address, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    address.get().serialize(serializer)
}

pub fn deserialize<'a, D>(deserializer: D) -> Result<Address, D::Error>
where
    D: Deserializer<'a>,
{
    deserializer.deserialize_any(AddressVisitor)
}

#[derive(Default)]
pub struct AddressVisitor;

impl<'b> Visitor<'b> for AddressVisitor {
    type Value = Address;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("an address")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: ::serde::de::Error,
    {
        Ok(Address {
            address: value.to_string(),
        })
    }
}

#[derive(Debug, PartialEq)]
pub enum Error {
    InvalidPublic,
    InvalidSecret,
    InvalidMessage,
    InvalidSignature,
    InvalidNetwork,
    InvalidChecksum,
    InvalidPrivate,
    InvalidAddress,
    FailedKeyGeneration,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let msg = match *self {
            Error::InvalidPublic => "Invalid Public",
            Error::InvalidSecret => "Invalid Secret",
            Error::InvalidMessage => "Invalid Message",
            Error::InvalidSignature => "Invalid Signature",
            Error::InvalidNetwork => "Invalid Network",
            Error::InvalidChecksum => "Invalid Checksum",
            Error::InvalidPrivate => "Invalid Private",
            Error::InvalidAddress => "Invalid Address",
            Error::FailedKeyGeneration => "Key generation failed",
        };

        msg.fmt(f)
    }
}

pub mod vec {
    use super::AddressVisitor;
    use crate::modules::primitives::address::Address;
    use serde::de::Visitor;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(addresses: &Vec<Address>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        addresses
            .iter()
            .map(|address| address.get())
            .collect::<Vec<_>>()
            .serialize(serializer)
    }

    pub fn deserialize<'a, D>(deserializer: D) -> Result<Vec<Address>, D::Error>
    where
        D: Deserializer<'a>,
    {
        <Vec<&'a str> as Deserialize>::deserialize(deserializer)?
            .into_iter()
            .map(|value| AddressVisitor::default().visit_str(value))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::primitives::address::Address;
    use serde_json;

    #[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
    struct TestStruct {
        #[serde(with = "crate::modules::types::address")]
        address: Address,
    }

    impl TestStruct {
        fn new(address: Address) -> Self {
            TestStruct { address: address }
        }
    }

    #[test]
    fn address_serialize() {
        let test = TestStruct::new(Address {
            address: "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa".to_string(),
        });
        assert_eq!(
            serde_json::to_string(&test).unwrap(),
            r#"{"address":"1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa"}"#
        );
    }

    #[test]
    fn address_deserialize() {
        let test = TestStruct::new(Address {
            address: "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa".to_string(),
        });
        assert_eq!(
            serde_json::from_str::<TestStruct>(
                r#"{"address":"1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa"}"#
            )
            .unwrap(),
            test
        );
    }
}
