//! Storage interface

pub mod schema;
pub mod traits;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Unknown database error")]
    Unknown,
}

pub type Result<T> = std::result::Result<T, Error>;
