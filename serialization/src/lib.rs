//! Blockchain data encoding and decoding tools

pub mod size_dependent;

// Re-export traits
pub use parity_scale_codec::{Codec, Decode, DecodeAll, Encode, Input, Output};

// Re-export types
pub use parity_scale_codec::Error;

// SCALE types and traits used by this crate only
use parity_scale_codec::{Compact, EncodeAsRef};
