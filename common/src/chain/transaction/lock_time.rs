use parity_scale_codec_derive::{Decode, Encode};

/// Represents a point in blockchain time, either in number of blocks or in real world time.
#[derive(Eq, PartialEq, Clone, Copy, Decode, Encode)]
pub enum BlockTime {
    /// Number of blocks since genesis
    Blocks(u32),
    /// Real world time
    Timestamp(Duration),
}
