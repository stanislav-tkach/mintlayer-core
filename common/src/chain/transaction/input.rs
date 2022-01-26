use crate::chain::transaction::{NoWitness, Transaction};
use crate::primitives::Id;
use parity_scale_codec::{Decode, Encode};

#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct OutPoint {
    id: Id<NoWitness<Transaction>>,
    index: u32,
}

impl OutPoint {
    pub fn new(prev_tx_id: Id<NoWitness<Transaction>>, output_index: u32) -> Self {
        OutPoint {
            id: prev_tx_id,
            index: output_index,
        }
    }

    pub fn get_tx_id(&self) -> Id<NoWitness<Transaction>> {
        self.id.clone()
    }

    pub fn get_output_index(&self) -> u32 {
        self.index
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct TxInput {
    // NOTE: The fields here have to be kept in sync with the Encode implementation
    // for NoWitness<&TxInput> below. Any changes here must be reflected there.
    outpoint: OutPoint,
    witness: Vec<u8>,
}

impl Encode for NoWitness<&TxInput> {
    fn size_hint(&self) -> usize {
        self.0.outpoint.size_hint()
    }

    fn encode_to<S: parity_scale_codec::Output + ?Sized>(&self, stream: &mut S) {
        self.0.outpoint.encode_to(stream)
        // This is the NoWitness encodig. Therefore, the witness field is not included.
    }
}

impl TxInput {
    pub fn new(prev_tx: Id<NoWitness<Transaction>>, output_index: u32, witness: Vec<u8>) -> Self {
        TxInput {
            outpoint: OutPoint::new(prev_tx, output_index),
            witness,
        }
    }

    pub fn get_outpoint(&self) -> &OutPoint {
        &self.outpoint
    }

    pub fn get_witness(&self) -> &Vec<u8> {
        &self.witness
    }
}
