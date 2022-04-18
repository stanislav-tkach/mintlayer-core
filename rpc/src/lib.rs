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

#[cfg(test)]
mod tests {
    use super::*;
    use jsonrpc_core::Params;
    use jsonrpc_core_client::transports::http;
    use jsonrpc_core_client::RawClient;

    #[tokio::test]
    async fn get_protocol_version() {
        let num_threads = 3;
        std::thread::spawn(move || start(&"127.0.0.1:3030".parse().unwrap(), num_threads));

        let client = http::connect::<RawClient>("http://127.0.0.1:3030")
            .await
            .expect("create client");
        assert_eq!(
            client.call_method("protocol_version", Params::None).await.unwrap(),
            "version1"
        );
    }
}
