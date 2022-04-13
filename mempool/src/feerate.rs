use common::primitives::amount::Amount;

lazy_static::lazy_static! {
    pub(crate) static ref INCREMENTAL_RELAY_FEE_RATE: FeeRate = FeeRate::new(1000);
    pub(crate) static ref INCREMENTAL_RELAY_THRESHOLD: FeeRate = FeeRate::new(500);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct FeeRate {
    tokens_per_kb: u128,
}

impl FeeRate {
    pub(crate) fn new(tokens_per_kb: u128) -> Self {
        Self { tokens_per_kb }
    }

    pub(crate) fn of_tx(fee: Amount, tx_size: usize) -> Self {
        let tokens_per_byte = Self::div_up(fee.into(), tx_size);
        Self {
            tokens_per_kb: tokens_per_byte * 1000,
        }
    }

    pub(crate) fn compute_fee(&self, size: usize) -> Amount {
        Amount::from_atoms(self.tokens_per_kb * u128::try_from(size).unwrap() / 1000)
    }

    pub(crate) fn tokens_per_kb(&self) -> u128 {
        self.tokens_per_kb
    }

    fn div_up(fee: u128, tx_size: usize) -> u128 {
        let tx_size = u128::try_from(tx_size).unwrap();
        (fee + tx_size - 1) / tx_size
    }
}

impl std::ops::Add for FeeRate {
    type Output = FeeRate;
    fn add(self, other: Self) -> Self::Output {
        let tokens_per_kb = self.tokens_per_kb + other.tokens_per_kb;
        FeeRate { tokens_per_kb }
    }
}

impl std::ops::Div<u128> for FeeRate {
    type Output = FeeRate;
    fn div(self, rhs: u128) -> Self::Output {
        FeeRate {
            tokens_per_kb: self.tokens_per_kb / rhs,
        }
    }
}
