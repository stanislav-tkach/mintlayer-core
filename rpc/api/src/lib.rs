mod traits;
use std::sync::Arc;
use traits::SpawnNamed;

use jsonrpc_core as rpc;
pub use jsonrpc_core::IoHandlerExtension as RpcExtension;
pub use jsonrpc_core::Metadata;

#[derive(Clone, Copy, Debug)]
pub enum DenyUnsafe {
    /// Denies only potentially unsafe RPCs.
    Yes,
    /// Allows calling every RPCs.
    No,
}

impl DenyUnsafe {
    /// Returns `Ok(())` if the RPCs considered unsafe are safe to call,
    /// otherwise returns `Err(UnsafeRpcError)`.
    pub fn check_if_safe(self) -> Result<(), UnsafeRpcError> {
        match self {
            DenyUnsafe::Yes => Err(UnsafeRpcError),
            DenyUnsafe::No => Ok(()),
        }
    }
}

/// Signifies whether an RPC considered unsafe is denied to be called externally.
#[derive(Debug)]
pub struct UnsafeRpcError;

impl std::fmt::Display for UnsafeRpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RPC call is unsafe to be called externally")
    }
}

impl std::error::Error for UnsafeRpcError {}

impl From<UnsafeRpcError> for rpc::Error {
    fn from(_: UnsafeRpcError) -> rpc::Error {
        rpc::Error::method_not_found()
    }
}

use futures::{
    task::{FutureObj, Spawn, SpawnError},
    FutureExt,
};

//pub mod chain;
//pub mod state;
//pub mod system;
//pub mod testing;

/// Task executor that is being used by RPC subscriptions.
#[derive(Clone)]
pub struct SubscriptionTaskExecutor(Arc<dyn SpawnNamed>);

impl SubscriptionTaskExecutor {
    /// Create a new `Self` with the given spawner.
    pub fn new(spawn: impl SpawnNamed + 'static) -> Self {
        Self(Arc::new(spawn))
    }
}

impl Spawn for SubscriptionTaskExecutor {
    fn spawn_obj(&self, future: FutureObj<'static, ()>) -> Result<(), SpawnError> {
        self.0.spawn(
            "mintlayer-rpc-subscription",
            Some("rpc"),
            future.map(drop).boxed(),
        );
        Ok(())
    }

    fn status(&self) -> Result<(), SpawnError> {
        Ok(())
    }
}
