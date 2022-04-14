#![allow(unused_imports)]

use jsonrpc_core::futures::{self, future, TryFutureExt};
use jsonrpc_core::types::Value;
use jsonrpc_core::Params;
use jsonrpc_core::{BoxFuture, IoHandler, Result};
use jsonrpc_core_client::transports::http;
use jsonrpc_core_client::RawClient;
use jsonrpc_derive::rpc;
use serde_json::value::Number;

/// Rpc trait
#[rpc]
pub trait Rpc {
    /// Returns a protocol version
    #[rpc(name = "protocolVersion")]
    fn protocol_version(&self) -> Result<String>;

    /// Adds two numbers and returns a result
    #[rpc(name = "add", alias("callAsyncMetaAlias"))]
    fn add(&self, a: u64, b: u64) -> Result<u64>;

    /// Performs asynchronous operation
    #[rpc(name = "callAsync")]
    fn call(&self, a: u64) -> BoxFuture<Result<String>>;
}

#[tokio::main]
async fn main() {
    let client = http::connect::<RpcClient>("http://127.0.0.1:3030")
        .await
        .expect("create client");

    let res = client.add(5, 6).await;

    println!("result: {:?}", res);
}
