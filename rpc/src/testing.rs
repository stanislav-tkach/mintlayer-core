use futures::{
    executor,
    task::{FutureObj, Spawn, SpawnError},
};

// Executor shared by all tests.
//
// This shared executor is used to prevent `Too many open files` errors
// on systems with a lot of cores.
lazy_static::lazy_static! {
    static ref EXECUTOR: executor::ThreadPool = executor::ThreadPool::new()
        .expect("Failed to create thread pool executor for tests");
}

/// Executor for use in testing
pub struct TaskExecutor;
impl Spawn for TaskExecutor {
    fn spawn_obj(&self, future: FutureObj<'static, ()>) -> Result<(), SpawnError> {
        EXECUTOR.spawn_ok(future);
        Ok(())
    }

    fn status(&self) -> Result<(), SpawnError> {
        Ok(())
    }
}
