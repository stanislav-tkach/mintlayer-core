use jsonrpc_core::futures::future;
use jsonrpc_core::{BoxFuture, IoHandler, Result};
use jsonrpc_derive::rpc;

use std::collections::HashMap;
use std::sync::{atomic, Arc, RwLock};

use jsonrpc_core::{Error, ErrorCode};
use jsonrpc_http_server::ServerBuilder;

#[rpc(server)]
pub trait Rpc {
    #[rpc(name = "protocolVersion")]
    fn protocol_version(&self) -> Result<String>;

    #[rpc(name = "add")]
    fn add(&self, a: u64, b: u64) -> Result<u64>;

    #[rpc(name = "callAsync")]
    fn call(&self, a: u64) -> BoxFuture<Result<String>>;
}

#[derive(Default)]
struct RpcImpl;
impl Rpc for RpcImpl {
    fn protocol_version(&self) -> Result<String> {
        Ok("version1".into())
    }

    fn add(&self, a: u64, b: u64) -> Result<u64> {
        Ok(a + b)
    }

    fn call(&self, _: u64) -> BoxFuture<Result<String>> {
        Box::pin(future::ready(Ok("OK".to_owned())))
    }
}

fn main() {
    let mut io = IoHandler::default();
    io.extend_with(RpcImpl::default().to_delegate());

    let server = ServerBuilder::new(io)
        .threads(3)
        .start_http(&"127.0.0.1:3030".parse().unwrap())
        .unwrap();

    server.wait();
}
