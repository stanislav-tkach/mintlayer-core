use common::{
    chain::{block::Block, SpendError, Spender, TxMainChainIndexError, TxMainChainPosition},
    primitives::{Amount, Id},
};
use thiserror::Error;

#[derive(Error, Debug, PartialEq, Eq, Clone)]
pub enum StateUpdateError {
    #[error("Blockchain storage error: {0}")]
    StorageError(chainstate_storage::Error),
    #[error("While connecting a block, transaction number `{0}` does not exist in block `{1}`")]
    TxNumWrongInBlockOnConnect(usize, Id<Block>),
    #[error("While disconnecting a block, transaction number `{0}` does not exist in block `{1}`")]
    TxNumWrongInBlockOnDisconnect(usize, Id<Block>),
    #[error("While disconnecting a block, transaction number `{0}` does not exist in block `{1}`")]
    InvariantErrorTxNumWrongInBlock(usize, Id<Block>),
    #[error("Outputs already in the inputs cache")]
    OutputAlreadyPresentInInputsCache,
    #[error("Block reward spent immaturely")]
    ImmatureBlockRewardSpend,
    #[error("Input was cached, but could not be found")]
    PreviouslyCachedInputNotFound,
    #[error("Input was cached, but it is erased")]
    PreviouslyCachedInputWasErased,
    #[error("Block disconnect already-unspent (invaraint broken)")]
    InvariantBrokenAlreadyUnspent,
    #[error("Source block index for block reward output not found")]
    InvariantBrokenSourceBlockIndexNotFound,
    #[error("Output is not found in the cache or database")]
    MissingOutputOrSpent,
    #[error("While connecting a block, output was erased in a previous step (possible in reorgs with no cache flushing)")]
    MissingOutputOrSpentOutputErasedOnConnect,
    #[error("While disconnecting a block, output was erased in a previous step (possible in reorgs with no cache flushing)")]
    MissingOutputOrSpentOutputErasedOnDisconnect,
    #[error("Attempt to print money (total inputs: `{0:?}` vs total outputs `{1:?}`")]
    AttemptToPrintMoney(Amount, Amount),
    #[error("Fee calculation failed (total inputs: `{0:?}` vs total outputs `{1:?}`")]
    TxFeeTotalCalcFailed(Amount, Amount),
    #[error("Output addition error")]
    OutputAdditionError,
    #[error("Signature verification failed in transaction")]
    SignatureVerificationFailed,
    #[error("Invalid output count")]
    InvalidOutputCount,
    #[error("Block distance calculation for maturity failed")]
    BlockHeightArithmeticError,
    #[error("Input addition error")]
    InputAdditionError,
    #[error("Double-spend attempt")]
    DoubleSpendAttempt(Spender),
    #[error("Input of tx {tx_id:?} has an out-of-range output index {source_output_index}")]
    OutputIndexOutOfRange {
        tx_id: Option<Spender>,
        source_output_index: usize,
    },
    #[error("Transaction index found but transaction not found")]
    InvariantErrorTransactionCouldNotBeLoaded(TxMainChainPosition),
    #[error("Transaction index for header found but header not found")]
    InvariantErrorHeaderCouldNotBeLoaded(Id<Block>),
    #[error("Addition of all fees in block `{0}` failed")]
    FailedToAddAllFeesOfBlock(Id<Block>),
    #[error("Block reward addition error for block {0}")]
    RewardAdditionError(Id<Block>),
    #[error("Serialization invariant failed for block `{0}`")]
    SerializationInvariantError(Id<Block>),
}

impl From<chainstate_storage::Error> for StateUpdateError {
    fn from(err: chainstate_storage::Error) -> Self {
        // On storage level called err.recoverable(), if an error is unrecoverable then it calls panic!
        // We don't need to cause panic here
        StateUpdateError::StorageError(err)
    }
}

impl From<SpendError> for StateUpdateError {
    fn from(err: SpendError) -> Self {
        match err {
            SpendError::AlreadySpent(spender) => StateUpdateError::DoubleSpendAttempt(spender),
            SpendError::AlreadyUnspent => StateUpdateError::InvariantBrokenAlreadyUnspent,
            SpendError::OutOfRange {
                tx_id,
                source_output_index,
            } => StateUpdateError::OutputIndexOutOfRange {
                tx_id,
                source_output_index,
            },
        }
    }
}

impl From<TxMainChainIndexError> for StateUpdateError {
    fn from(err: TxMainChainIndexError) -> Self {
        match err {
            TxMainChainIndexError::InvalidOutputCount => StateUpdateError::InvalidOutputCount,
            TxMainChainIndexError::SerializationInvariantError(block_id) => {
                StateUpdateError::SerializationInvariantError(block_id)
            }
            TxMainChainIndexError::InvalidTxNumberForBlock(tx_num, block_id) => {
                StateUpdateError::InvariantErrorTxNumWrongInBlock(tx_num, block_id)
            }
        }
    }
}
