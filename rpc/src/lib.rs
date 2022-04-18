use std::net::SocketAddr;

use jsonrpc_core::IoHandler;
use jsonrpc_core::Result;
use jsonrpc_derive::rpc;
use jsonrpc_http_server::ServerBuilder;

#[rpc(server)]
pub trait Rpc {
    #[rpc(name = "protocol_version")]
    fn protocol_version(&self) -> Result<String>;
}

#[derive(Default)]
struct RpcImpl;
impl Rpc for RpcImpl {
    fn protocol_version(&self) -> Result<String> {
        Ok("version1".into())
    }
}

pub fn start(addr: &SocketAddr, num_threads: usize) -> anyhow::Result<()> {
    let mut io = IoHandler::default();
    io.extend_with(RpcImpl::default().to_delegate());

    let server = ServerBuilder::new(io).threads(num_threads).start_http(addr)?;
    server.wait();
    Ok(())
}
