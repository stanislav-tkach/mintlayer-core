use crypto::hash::StreamHasher;

use crate::chain::transaction::Transaction;
use crate::primitives::{id, Id, Idable};
use parity_scale_codec::{Decode, Encode};

#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct OutPoint {
    id: Id<Transaction>,
    index: u32,
}

impl OutPoint {
    pub fn new(prev_tx_id: Id<Transaction>, output_index: u32) -> Self {
        OutPoint {
            id: prev_tx_id,
            index: output_index,
        }
    }

    pub fn get_tx_id(&self) -> Id<Transaction> {
        self.id.clone()
    }

    pub fn get_output_index(&self) -> u32 {
        self.index
    }
}

impl Idable<OutPoint> for OutPoint {
    fn get_id(&self) -> Id<Self> {
        let mut hash_stream = id::DefaultHashAlgoStream::new();
        id::hash_encoded_to(&self.id, &mut hash_stream);
        id::hash_encoded_to(&self.index, &mut hash_stream);
        Id::new(&hash_stream.finalize().into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct TxInput {
    outpoint: OutPoint,
    witness: Vec<u8>,
}

impl TxInput {
    pub fn new(prev_tx_id: Id<Transaction>, output_index: u32, witness: Vec<u8>) -> Self {
        TxInput {
            outpoint: OutPoint::new(prev_tx_id, output_index),
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
