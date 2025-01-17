//! Node configuration options

use std::ffi::OsString;
use std::net::SocketAddr;
use std::path::PathBuf;
use strum::VariantNames;

use common::chain::config::ChainType;

/// Mintlayer node executable
#[derive(clap::Parser, Debug)]
#[clap(author, version, about)]
pub struct Options {
    /// Where to write logs
    #[clap(long, value_name = "PATH")]
    pub log_path: Option<PathBuf>,

    /// Address to bind RPC to
    #[clap(long, value_name = "ADDR", default_value = "127.0.0.1:3030")]
    pub rpc_addr: SocketAddr,

    /// Blockchain type
    #[clap(long, possible_values = ChainType::VARIANTS, default_value = "mainnet")]
    pub net: ChainType,

    /// Address to bind P2P to
    #[clap(long, value_name = "ADDR", default_value = "/ip6/::1/tcp/3031")]
    pub p2p_addr: String,
}

impl Options {
    pub fn from_args<A: Into<OsString> + Clone>(args: impl IntoIterator<Item = A>) -> Self {
        clap::Parser::parse_from(args)
    }
}
