#![allow(unused_imports)]

use jsonrpc_core::futures::{self, future, TryFutureExt};
use jsonrpc_core::types::Value;
use jsonrpc_core::Params;
use jsonrpc_core::{BoxFuture, IoHandler, Result};
use jsonrpc_core_client::transports::http;
use jsonrpc_core_client::RawClient;
use jsonrpc_derive::rpc;
use serde_json::value::Number;

#[tokio::main]
async fn main() {
    let client = http::connect::<RawClient>("http://127.0.0.1:3030")
        .await
        .expect("create client");
    let res = client
        .call_method(
            "add",
            Params::Array(vec![
                Value::Number(Number::from(120)),
                Value::Number(Number::from(360)),
            ]),
        )
        .await;

    println!("result: {:?}", res);
}
