const NODE_EXECUTABLE: &str = env!("CARGO_BIN_EXE_node");

use common::{chain::block::{Block, ConsensusData, consensus_data::PoWData}, primitives::{Compact, Id, Idable}};
use jsonrpsee::{core::client::ClientT, http_client, rpc_params};

struct Node {
    process: std::process::Child,
    rpc_client: http_client::HttpClient,
}

impl Node {
    /// Spawn a new node, giving a handle
    fn spawn(rpc_port: u16) -> Self {
        // Spawn the process
        let process = std::process::Command::new(NODE_EXECUTABLE)
            .arg(format!("--rpc-addr=127.0.0.1:{}", rpc_port))
            .spawn()
            .expect("Node failed to run");

        // Wait a bit for the node to start
        std::thread::sleep(std::time::Duration::from_millis(300));

        // Establish RPC connection
        let rpc_client = http_client::HttpClientBuilder::default()
            .build(format!("http://127.0.0.1:{}", rpc_port))
            .expect("Failed to establish RPC connection");

        Self { process, rpc_client }
    }

    /// Perform an RPC call to the node
    fn rpc<'a, R: serde::de::DeserializeOwned>(
        &self,
        method: &'a str,
        params: Option<jsonrpsee::types::ParamsSer<'a>>,
    ) -> Result<R, jsonrpsee::types::error::CallError> {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let result = rt.block_on(self.rpc_client.request(method, params));
        result.map_err(|err| match err {
            rpc::Error::Call(e) => e,
            e => panic!("RPC error: {}", e),
        })
    }
}

impl Drop for Node {
    fn drop(&mut self) {
        let _ = self.rpc::<()>("node_shutdown", rpc_params!());
        let status = self.process.wait().expect("Node not running");
        assert_eq!(status.code(), Some(0), "Node exitted with failure");
    }
}

#[test]
fn startup_and_shutdown() {
    let _node = Node::spawn(3310);
}

#[test]
fn block_submission() {
    let node = Node::spawn(3311);

    let genesis_id: Id<Block> =
        node.rpc("consensus_best_block_id", rpc_params!()).expect("cannot get genesis ID");

    /*
    let bits = Compact(0x21_00ffff);
    //println!("cmpt: {:?}", common::Uint256::try_from(bits).unwrap());
    let mut nonce = 0;
    let block = loop {
        let consensus_data = ConsensusData::PoW(PoWData::new(bits, nonce, vec![]));
        let block = Block::new(vec![], Some(genesis_id.clone()), 0, consensus_data).expect("Block creation");
        let id = block.get_id().get();
        if id.as_ref().ends_with(&[0, 0, 0, 0]) {
            break block;
        }
        nonce += 1;
    };
    println!("blid: 0x{:x}", block.get_id().get());
    let blk_hex = hex::encode(&serialization::Encode::encode(&block));
    node.rpc::<()>("consensus_submit_block", rpc_params!(blk_hex)).expect("block submission fail");
    */
}
