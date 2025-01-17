use itertools::Itertools;

use crypto::key::{KeyKind, PrivateKey};
use script::Script;

use super::utils::*;
use crate::{
    address::pubkeyhash::PublicKeyHash,
    chain::{
        signature::{
            sighashtype::{OutputsMode, SigHashType},
            TransactionSigError,
        },
        Destination,
    },
    primitives::{Id, H256},
};

// Generate a transaction with a different number of inputs and outputs, then sign it as a whole.
// For `ALL`, `ALL | ANYONECANPAY`, `NONE` and `NONE | ANYONECANPAY` it should succeed in all cases
// except for `ScriptHash` and `AnyoneCanSpend` destinations.
#[test]
fn sign_and_verify_all_and_none() {
    let test_data = [(0, 31), (31, 0), (20, 3), (3, 20)];
    let (private_key, public_key) = PrivateKey::new(KeyKind::RistrettoSchnorr);

    for ((destination, sighash_type), (inputs, outputs)) in destinations(public_key)
        .cartesian_product(sig_hash_types().filter(|t| t.outputs_mode() != OutputsMode::Single))
        .cartesian_product(test_data)
    {
        let mut tx = generate_unsigned_tx(&destination, inputs, outputs).unwrap();
        let res = sign_whole_tx(&mut tx, &private_key, sighash_type, &destination);
        // `sign_whole_tx` does nothing if there no inputs.
        if destination == Destination::AnyoneCanSpend && inputs > 0 {
            assert_eq!(
                res,
                Err(TransactionSigError::AttemptedToProduceSignatureForAnyoneCanSpend)
            );
        } else if matches!(destination, Destination::ScriptHash(_)) && inputs > 0 {
            // This should be updated after ScriptHash support is implemented.
            assert_eq!(res, Err(TransactionSigError::Unsupported));
        } else {
            assert_eq!(Ok(()), res, "{sighash_type:?} {destination:?}");
            assert_eq!(
                Ok(()),
                verify_signed_tx(&tx, &destination),
                "{sighash_type:?} {destination:?}"
            );
        }
    }
}

// Same as `sign_and_verify_all_and_none` but for `SINGLE` and `SINGLE | ANYONECANPAY` signature
// hash types.
#[test]
fn sign_and_verify_single() {
    let (private_key, public_key) = PrivateKey::new(KeyKind::RistrettoSchnorr);
    let test_data = [
        // SigHashType::SINGLE. Destination = PubKey.
        (
            Destination::PublicKey(public_key.clone()),
            SigHashType::try_from(SigHashType::SINGLE).unwrap(),
            0,
            31,
            Ok(()),
        ),
        (
            Destination::PublicKey(public_key.clone()),
            SigHashType::try_from(SigHashType::SINGLE).unwrap(),
            31,
            0,
            Err(TransactionSigError::InvalidInputIndex(0, 0)),
        ),
        (
            Destination::PublicKey(public_key.clone()),
            SigHashType::try_from(SigHashType::SINGLE).unwrap(),
            21,
            3,
            Err(TransactionSigError::InvalidInputIndex(3, 3)),
        ),
        (
            Destination::PublicKey(public_key.clone()),
            SigHashType::try_from(SigHashType::SINGLE).unwrap(),
            3,
            21,
            Ok(()),
        ),
        // SigHashType::SINGLE | SigHashType::ANYONECANPAY. Destination = PubKey.
        (
            Destination::PublicKey(public_key.clone()),
            SigHashType::try_from(SigHashType::SINGLE | SigHashType::ANYONECANPAY).unwrap(),
            0,
            33,
            Ok(()),
        ),
        (
            Destination::PublicKey(public_key.clone()),
            SigHashType::try_from(SigHashType::SINGLE | SigHashType::ANYONECANPAY).unwrap(),
            21,
            0,
            Err(TransactionSigError::InvalidInputIndex(0, 0)),
        ),
        (
            Destination::PublicKey(public_key.clone()),
            SigHashType::try_from(SigHashType::SINGLE | SigHashType::ANYONECANPAY).unwrap(),
            15,
            33,
            Ok(()),
        ),
        (
            Destination::PublicKey(public_key.clone()),
            SigHashType::try_from(SigHashType::SINGLE | SigHashType::ANYONECANPAY).unwrap(),
            21,
            7,
            Err(TransactionSigError::InvalidInputIndex(7, 7)),
        ),
        // SigHashType::SINGLE. Destination = Address.
        (
            Destination::Address(PublicKeyHash::from(&public_key)),
            SigHashType::try_from(SigHashType::SINGLE).unwrap(),
            0,
            33,
            Ok(()),
        ),
        (
            Destination::Address(PublicKeyHash::from(&public_key)),
            SigHashType::try_from(SigHashType::SINGLE).unwrap(),
            21,
            0,
            Err(TransactionSigError::InvalidInputIndex(0, 0)),
        ),
        (
            Destination::Address(PublicKeyHash::from(&public_key)),
            SigHashType::try_from(SigHashType::SINGLE).unwrap(),
            15,
            33,
            Ok(()),
        ),
        (
            Destination::Address(PublicKeyHash::from(&public_key)),
            SigHashType::try_from(SigHashType::SINGLE).unwrap(),
            21,
            7,
            Err(TransactionSigError::InvalidInputIndex(7, 7)),
        ),
        // SigHashType::SINGLE | SigHashType::ANYONECANPAY. Destination = Address.
        (
            Destination::Address(PublicKeyHash::from(&public_key)),
            SigHashType::try_from(SigHashType::SINGLE | SigHashType::ANYONECANPAY).unwrap(),
            0,
            33,
            Ok(()),
        ),
        (
            Destination::Address(PublicKeyHash::from(&public_key)),
            SigHashType::try_from(SigHashType::SINGLE | SigHashType::ANYONECANPAY).unwrap(),
            21,
            0,
            Err(TransactionSigError::InvalidInputIndex(0, 0)),
        ),
        (
            Destination::Address(PublicKeyHash::from(&public_key)),
            SigHashType::try_from(SigHashType::SINGLE | SigHashType::ANYONECANPAY).unwrap(),
            15,
            33,
            Ok(()),
        ),
        (
            Destination::Address(PublicKeyHash::from(&public_key)),
            SigHashType::try_from(SigHashType::SINGLE | SigHashType::ANYONECANPAY).unwrap(),
            21,
            7,
            Err(TransactionSigError::InvalidInputIndex(7, 7)),
        ),
        // SigHashType::SINGLE. Destination = AnyoneCanSpend.
        (
            Destination::AnyoneCanSpend,
            SigHashType::try_from(SigHashType::SINGLE).unwrap(),
            21,
            33,
            Err(TransactionSigError::AttemptedToProduceSignatureForAnyoneCanSpend),
        ),
        // SigHashType::SINGLE | SigHashType::ANYONECANPAY. Destination = AnyoneCanSpend.
        (
            Destination::AnyoneCanSpend,
            SigHashType::try_from(SigHashType::SINGLE | SigHashType::ANYONECANPAY).unwrap(),
            21,
            33,
            Err(TransactionSigError::AttemptedToProduceSignatureForAnyoneCanSpend),
        ),
        // SigHashType::SINGLE. Destination = ScriptHash.
        // This is currently unsupported, so test should be updated in the future.
        (
            Destination::ScriptHash(Id::<Script>::from(H256::random())),
            SigHashType::try_from(SigHashType::SINGLE).unwrap(),
            21,
            33,
            Err(TransactionSigError::Unsupported),
        ),
        // SigHashType::SINGLE | SigHashType::ANYONECANPAY. Destination = ScriptHash
        // This is currently unsupported, so test should be updated in the future.
        (
            Destination::ScriptHash(Id::<Script>::from(H256::random())),
            SigHashType::try_from(SigHashType::SINGLE | SigHashType::ANYONECANPAY).unwrap(),
            21,
            33,
            Err(TransactionSigError::Unsupported),
        ),
    ];

    for (destination, sighash_type, inputs, outputs, expected) in test_data.into_iter() {
        let mut tx = generate_unsigned_tx(&destination, inputs, outputs).unwrap();
        match sign_whole_tx(&mut tx, &private_key, sighash_type, &destination) {
            Ok(_) => assert_eq!(
                verify_signed_tx(&tx, &destination),
                expected,
                "{sighash_type:X?}, {destination:?}"
            ),
            Err(err) => assert_eq!(Err(err), expected, "{sighash_type:X?}, {destination:?}"),
        }
    }
}
