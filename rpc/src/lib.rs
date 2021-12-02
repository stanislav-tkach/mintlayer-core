
use futures::{
	task::{FutureObj, Spawn, SpawnError},
	FutureExt,
};
use rpc_api::traits::SpawnNamed;
use std::sync::Arc;

pub use jsonrpc_core::IoHandlerExtension as RpcExtension;
pub use rpc_api::{DenyUnsafe, Metadata};

//pub mod chain;
//pub mod state;
//pub mod system;

#[cfg(any(test, feature = "test-helpers"))]
pub mod testing;

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
		self.0
			.spawn("substrate-rpc-subscription", Some("rpc"), future.map(drop).boxed());
		Ok(())
	}

	fn status(&self) -> Result<(), SpawnError> {
		Ok(())
	}
}