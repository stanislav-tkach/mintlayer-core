use common::primitives::amount::Amount;

use crate::error::TxValidationError;

lazy_static::lazy_static! {
    pub(crate) static ref INCREMENTAL_RELAY_FEE_RATE: FeeRate = FeeRate::new(1000);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct FeeRate {
    tokens_per_byte: Amount,
}

impl FeeRate {
    pub(crate) fn new(tokens_per_byte: impl Into<Amount>) -> Self {
        let tokens_per_byte: Amount = tokens_per_byte.into();
        Self { tokens_per_byte }
    }

    pub(crate) fn compute_fee(&self, size: usize) -> Result<Amount, TxValidationError> {
        (self.tokens_per_byte * Amount::from(size as u128)).ok_or(TxValidationError::FeeRateError)
    }

    pub(crate) fn tokens_per_byte(&self) -> Amount {
        self.tokens_per_byte
    }
}

impl std::ops::Add for FeeRate {
    type Output = Option<FeeRate>;
    fn add(self, other: Self) -> Self::Output {
        let tokens_per_byte = self.tokens_per_byte + other.tokens_per_byte;
        tokens_per_byte.map(|tokens_per_byte| FeeRate { tokens_per_byte })
    }
}

impl std::ops::Div for FeeRate {
    type Output = Option<FeeRate>;
    fn div(self, other: Self) -> Self::Output {
        let tokens_per_byte = self.tokens_per_byte / other.tokens_per_byte;
        tokens_per_byte.map(|tokens_per_byte| FeeRate { tokens_per_byte })
    }
}
