use common::primitives::amount::Amount;

use crate::error::TxValidationError;

lazy_static::lazy_static! {
    pub(crate) static ref INCREMENTAL_RELAY_FEE_RATE: FeeRate = FeeRate::new(1000);
    pub(crate) static ref INCREMENTAL_RELAY_THRESHOLD: FeeRate = FeeRate::new(500);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct FeeRate {
    tokens_per_kb: Amount,
}

impl FeeRate {
    pub(crate) fn new(tokens_per_kb: impl Into<Amount>) -> Self {
        let tokens_per_kb: Amount = tokens_per_kb.into();
        Self { tokens_per_kb }
    }

    pub(crate) fn of_tx(fee: Amount, tx_size: usize) -> Result<Self, TxValidationError> {
        let tokens_per_byte = Self::div_up(fee, tx_size)?;
        Ok(Self {
            tokens_per_kb: (tokens_per_byte * Amount::from(1000)).unwrap(),
        })
    }

    pub(crate) fn compute_fee(&self, size: usize) -> Result<Amount, TxValidationError> {
        ((self.tokens_per_kb * Amount::from(u128::try_from(size).unwrap())).unwrap()
            / Amount::from(1000))
        .ok_or(TxValidationError::FeeRateError)
    }

    pub(crate) fn tokens_per_kb(&self) -> Amount {
        self.tokens_per_kb
    }

    fn div_up(fee: Amount, tx_size: usize) -> Result<Amount, TxValidationError> {
        let tx_size = tx_size as u128;
        (((fee + Amount::from(tx_size)).ok_or(TxValidationError::FeeRateError)? - Amount::from(1))
            .ok_or(TxValidationError::FeeRateError)?
            / Amount::from(tx_size))
        .ok_or(TxValidationError::FeeRateError)
    }
}

impl std::ops::Add for FeeRate {
    type Output = Option<FeeRate>;
    fn add(self, other: Self) -> Self::Output {
        let tokens_per_byte = self.tokens_per_kb + other.tokens_per_kb;
        tokens_per_byte.map(|tokens_per_byte| FeeRate {
            tokens_per_kb: tokens_per_byte,
        })
    }
}

impl std::ops::Div for FeeRate {
    type Output = Option<FeeRate>;
    fn div(self, other: Self) -> Self::Output {
        let tokens_per_byte = self.tokens_per_kb / other.tokens_per_kb;
        tokens_per_byte.map(|tokens_per_byte| FeeRate {
            tokens_per_kb: tokens_per_byte,
        })
    }
}
