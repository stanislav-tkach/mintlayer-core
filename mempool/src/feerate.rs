use std::cell::Cell;
use std::time::Duration;
use std::time::Instant;

use common::primitives::amount::Amount;

use crate::error::TxValidationError;
use crate::pool::MemoryUsage;

const ROLLING_FEE_HALF_LIFE: usize = 60 * 60 * 12;

lazy_static::lazy_static! {
    pub(crate) static ref INCREMENTAL_RELAY_FEE_RATE: FeeRate = FeeRate::new(1000);
}

#[derive(Debug)]
pub(crate) struct RollingFeeRate {
    inner: Cell<RollingFeeRateInner>,
    memory_usage_estimator: Option<MemoryUsage>,
    size_limit: usize,
}

#[derive(Clone, Copy, Debug)]
struct RollingFeeRateInner {
    block_since_last_rolling_fee_bump: bool,
    rolling_minimum_fee_rate: FeeRate,
    last_rolling_fee_update: Instant,
}

impl RollingFeeRateInner {
    fn decay_fee(mut self, halflife: usize) -> Result<Self, TxValidationError> {
        self.rolling_minimum_fee_rate = (self.rolling_minimum_fee_rate.tokens_per_byte
            / (Amount::from(2)
                .pow((self.last_rolling_fee_update.elapsed().as_secs()) as usize / halflife))
            .ok_or(TxValidationError::FeeRateError)?)
        .map(FeeRate::new)
        .ok_or(TxValidationError::FeeRateError)?;
        self.last_rolling_fee_update = Instant::now();
        Ok(self)
    }

    fn drop_fee(mut self) -> Self {
        self.rolling_minimum_fee_rate = FeeRate::new(0);
        self
    }
}

impl RollingFeeRate {
    pub(crate) fn new(memory_usage_estimator: Option<MemoryUsage>, size_limit: usize) -> Self {
        let inner = Cell::new(RollingFeeRateInner {
            block_since_last_rolling_fee_bump: false,
            rolling_minimum_fee_rate: FeeRate::new(0),
            last_rolling_fee_update: Instant::now(),
        });
        Self {
            inner,
            memory_usage_estimator,
            size_limit,
        }
    }

    // TODO need to update halflife according to memory usage and size limits
    // TODO update this struct when TX is finalized
    // TODO update this struct when a new block is processed
    // TODO this needs to be tested

    pub(crate) fn get_min_fee_rate(&self) -> FeeRate {
        self.inner.get().rolling_minimum_fee_rate
    }

    fn decay_fee(&self) -> Result<(), TxValidationError> {
        self.inner.set(self.inner.get().decay_fee(self.halflife())?);
        Ok(())
    }

    fn drop_fee(&self) {
        self.inner.set(self.inner.get().drop_fee());
    }

    fn block_since_last_rolling_fee_bump(&self) -> bool {
        self.inner.get().block_since_last_rolling_fee_bump
    }

    fn last_rolling_fee_update(&self) -> Instant {
        self.inner.get().last_rolling_fee_update
    }

    pub(crate) fn update_min_fee_rate(&mut self, rate: FeeRate) {
        let rolling_fee_rate = self.inner.get_mut();
        rolling_fee_rate.rolling_minimum_fee_rate = rate;
        rolling_fee_rate.block_since_last_rolling_fee_bump = false;
    }

    fn halflife(&self) -> usize {
        if let Some(memory_usage_estimator) = &self.memory_usage_estimator {
            let mem_usage = memory_usage_estimator.get_memory_usage();
            if mem_usage < self.size_limit / 4 {
                ROLLING_FEE_HALF_LIFE / 4
            } else if mem_usage < self.size_limit / 2 {
                ROLLING_FEE_HALF_LIFE / 2
            } else {
                ROLLING_FEE_HALF_LIFE
            }
        } else {
            ROLLING_FEE_HALF_LIFE
        }
    }

    pub(crate) fn get_update_min_fee_rate(&self) -> Result<FeeRate, TxValidationError> {
        if !self.block_since_last_rolling_fee_bump() || self.get_min_fee_rate() == FeeRate::new(0) {
            return Ok(self.get_min_fee_rate());
        } else if Instant::now() > self.last_rolling_fee_update() + Duration::from_secs(10) {
            // Decay the rolling fee
            self.decay_fee()?;

            if self.get_min_fee_rate()
                < (*INCREMENTAL_RELAY_FEE_RATE / FeeRate::new(2)).expect("not division by zero")
            {
                self.drop_fee();
                return Ok(self.get_min_fee_rate());
            }
        }

        Ok(std::cmp::max(
            self.inner.get().rolling_minimum_fee_rate,
            *INCREMENTAL_RELAY_FEE_RATE,
        ))
    }
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
