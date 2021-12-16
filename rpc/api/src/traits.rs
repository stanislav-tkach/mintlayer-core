use dyn_clone::DynClone;
/// Something that can spawn tasks (blocking and non-blocking) with an assigned name
/// and optional group.
pub trait SpawnNamed: DynClone + Send + Sync {
    /// Spawn the given blocking future.
    ///
    /// The given `group` and `name` is used to identify the future in tracing.
    fn spawn_blocking(
        &self,
        name: &'static str,
        group: Option<&'static str>,
        future: futures::future::BoxFuture<'static, ()>,
    );
    /// Spawn the given non-blocking future.
    ///
    /// The given `group` and `name` is used to identify the future in tracing.
    fn spawn(
        &self,
        name: &'static str,
        group: Option<&'static str>,
        future: futures::future::BoxFuture<'static, ()>,
    );
}

dyn_clone::clone_trait_object!(SpawnNamed);

impl SpawnNamed for Box<dyn SpawnNamed> {
    fn spawn_blocking(
        &self,
        name: &'static str,
        group: Option<&'static str>,
        future: futures::future::BoxFuture<'static, ()>,
    ) {
        (**self).spawn_blocking(name, group, future)
    }
    fn spawn(
        &self,
        name: &'static str,
        group: Option<&'static str>,
        future: futures::future::BoxFuture<'static, ()>,
    ) {
        (**self).spawn(name, group, future)
    }
}

/// Something that can spawn essential tasks (blocking and non-blocking) with an assigned name
/// and optional group.
///
/// Essential tasks are special tasks that should take down the node when they end.
pub trait SpawnEssentialNamed: DynClone + Send + Sync {
    /// Spawn the given blocking future.
    ///
    /// The given `group` and `name` is used to identify the future in tracing.
    fn spawn_essential_blocking(
        &self,
        name: &'static str,
        group: Option<&'static str>,
        future: futures::future::BoxFuture<'static, ()>,
    );
    /// Spawn the given non-blocking future.
    ///
    /// The given `group` and `name` is used to identify the future in tracing.
    fn spawn_essential(
        &self,
        name: &'static str,
        group: Option<&'static str>,
        future: futures::future::BoxFuture<'static, ()>,
    );
}

dyn_clone::clone_trait_object!(SpawnEssentialNamed);

impl SpawnEssentialNamed for Box<dyn SpawnEssentialNamed> {
    fn spawn_essential_blocking(
        &self,
        name: &'static str,
        group: Option<&'static str>,
        future: futures::future::BoxFuture<'static, ()>,
    ) {
        (**self).spawn_essential_blocking(name, group, future)
    }

    fn spawn_essential(
        &self,
        name: &'static str,
        group: Option<&'static str>,
        future: futures::future::BoxFuture<'static, ()>,
    ) {
        (**self).spawn_essential(name, group, future)
    }
}
