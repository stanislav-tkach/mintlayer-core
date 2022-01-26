use crate::chain::transaction::Transaction;
use crate::primitives::{Id, Idable, H256};
use parity_scale_codec_derive::{Decode as DecodeDer, Encode as EncodeDer};

#[derive(Debug, Clone, PartialEq, Eq, EncodeDer, DecodeDer)]
pub struct BlockHeader {
    pub(super) hash_prev_block: Id<super::Block>,
    pub(super) tx_merkle_root: H256,
    pub(super) witness_merkle_root: H256,
    pub(super) time: u32,
    pub(super) consensus_data: Vec<u8>,
}

// Block ID is determined by hashing the header
derive_idable_via_encode!(BlockHeader, super::Block, |b| (BlockV1::VERSION_BYTE, b));

#[derive(Debug, Clone, PartialEq, Eq, EncodeDer, DecodeDer)]
pub struct BlockV1 {
    pub(super) header: BlockHeader,
    pub(super) transactions: Vec<Transaction>,
}

impl BlockV1 {
    // This has to be the same its index in the Block enum
    pub const VERSION_BYTE: u8 = 0x01;

    pub fn get_tx_merkle_root(&self) -> H256 {
        self.header.tx_merkle_root
    }

    pub fn get_witness_merkle_root(&self) -> H256 {
        self.header.witness_merkle_root
    }

    pub fn get_header(&self) -> &BlockHeader {
        &self.header
    }

    pub fn update_consensus_data(&mut self, consensus_data: Vec<u8>) {
        self.header.consensus_data = consensus_data;
    }

    pub fn get_block_time(&self) -> u32 {
        self.header.time
    }

    pub fn get_transactions(&self) -> &Vec<Transaction> {
        &self.transactions
    }

    pub fn get_prev_block_id(&self) -> &Id<super::Block> {
        &self.header.hash_prev_block
    }
}

impl Idable for BlockV1 {
    type Tag = super::Block;
    fn get_id(&self) -> Id<Self::Tag> {
        // Block ID is just the hash of its header. The transaction list is committed to by the
        // inclusion of transaction Merkle root in the header. We also include the version number.
        self.header.get_id()
    }
}
