pub use crate::chain::transaction::input::*;
pub use crate::chain::transaction::output::*;
pub use crate::chain::transaction::NoWitness;
pub use crate::chain::transaction::TransactionCreationError;
use parity_scale_codec::{Decode, Encode};

use super::Transaction;

/// Transaction, format version 1.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct TransactionV1 {
    // NOTE: The fields here have to be kept in sync with the Encode implementation
    // for NoWitness<&TransactionV1> below. Any changes here must be reflected there.
    flags: u32,
    inputs: Vec<TxInput>,
    outputs: Vec<TxOutput>,
    lock_time: u32,
}

impl Encode for NoWitness<&TransactionV1> {
    fn encode_to<S: parity_scale_codec::Output + ?Sized>(&self, stream: &mut S) {
        let inputs: Vec<NoWitness<&TxInput>> = self.0.inputs.iter().map(NoWitness).collect();
        let data = (self.0.flags, &inputs, &self.0.outputs, self.0.lock_time);
        data.encode_to(stream)
    }
}

// We prepend 1u8 to the encoding to get an ID to add transaction format version.
derive_idable_via_encode!(TransactionV1, Transaction, |tx| (Self::VERSION_BYTE, tx));
derive_idable_via_encode!(NoWitness<&TransactionV1>, NoWitness<Transaction>,
                          |tx| (TransactionV1::VERSION_BYTE, tx));

impl TransactionV1 {
    // This has to be the same its index in the Transaction enum
    pub const VERSION_BYTE: u8 = 0x01;

    pub fn new(
        flags: u32,
        inputs: Vec<TxInput>,
        outputs: Vec<TxOutput>,
        lock_time: u32,
    ) -> Result<Self, TransactionCreationError> {
        let tx = TransactionV1 {
            flags,
            inputs,
            outputs,
            lock_time,
        };
        Ok(tx)
    }

    pub fn is_replaceable(&self) -> bool {
        (self.flags & 1) != 0
    }

    pub fn get_flags(&self) -> u32 {
        self.flags
    }

    pub fn get_inputs(&self) -> &Vec<TxInput> {
        &self.inputs
    }

    pub fn get_outputs(&self) -> &Vec<TxOutput> {
        &self.outputs
    }

    pub fn get_lock_time(&self) -> u32 {
        self.lock_time
    }
}
