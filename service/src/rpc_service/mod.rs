use crate::rpc;
use crate::rpc_server;

#[derive(Clone)]

pub struct RpcHandlers(Arc<jsonrpc_core::MetaIoHandler<rpc::Metadata, rpc_server::RpcMiddleware>>);

impl RpcHandlers {
    /// Starts an RPC query.
    ///
    /// The query is passed as a string and must be a JSON text similar to what an HTTP client
    /// would for example send.
    ///
    /// Returns a `Future` that contains the optional response.
    ///
    /// If the request subscribes you to events, the `Sender` in the `RpcSession` object is used to
    /// send back spontaneous events.
    pub fn rpc_query(
        &self,
        mem: &RpcSession,
        request: &str,
    ) -> Pin<Box<dyn Future<Output = Option<String>> + Send>> {
        self.0.handle_request(request, mem.metadata.clone()).boxed()
    }

    /// Provides access to the underlying `MetaIoHandler`
    pub fn io_handler(
        &self,
    ) -> Arc<jsonrpc_core::MetaIoHandler<rpc::Metadata, rpc_server::RpcMiddleware>> {
        self.0.clone()
    }
}

/// An RPC session. Used to perform in-memory RPC queries (ie. RPC queries that don't go through
/// the HTTP or WebSockets server).
#[derive(Clone)]
pub struct RpcSession {
    metadata: rpc::Metadata,
}

impl RpcSession {
    /// Creates an RPC session.
    ///
    /// The `sender` is stored inside the `RpcSession` and is used to communicate spontaneous JSON
    /// messages.
    ///
    /// The `RpcSession` must be kept alive in order to receive messages on the sender.
    pub fn new(sender: futures::channel::mpsc::UnboundedSender<String>) -> RpcSession {
        RpcSession {
            metadata: sender.into(),
        }
    }
}

// Wrapper for HTTP and WS servers that makes sure they are properly shut down.
mod waiting {
    pub struct HttpServer(pub Option<rpc_server::HttpServer>);
    impl Drop for HttpServer {
        fn drop(&mut self) {
            if let Some(server) = self.0.take() {
                server.close_handle().close();
                server.wait();
            }
        }
    }

    pub struct IpcServer(pub Option<rpc_server::IpcServer>);
    impl Drop for IpcServer {
        fn drop(&mut self) {
            if let Some(server) = self.0.take() {
                server.close_handle().close();
                let _ = server.wait();
            }
        }
    }

    pub struct WsServer(pub Option<rpc_server::WsServer>);
    impl Drop for WsServer {
        fn drop(&mut self) {
            if let Some(server) = self.0.take() {
                server.close_handle().close();
                let _ = server.wait();
            }
        }
    }
}

/****************************************************
 Starts RPC servers that run in their own thread,
 and returns an opaque object that keeps them alive.
*****************************************************/

fn start_rpc_servers<
    H: FnMut(
        rpc::DenyUnsafe,
        rpc_server::RpcMiddleware,
    ) -> Result<rpc_server::RpcHandler<rpc::Metadata>, Error>,
>(
    config: &Configuration,
    mut gen_handler: H,
    rpc_metrics: Option<rpc_server::RpcMetrics>,
    server_metrics: rpc_server::ServerMetrics,
) -> Result<Box<dyn std::any::Any + Send>, Error> {
    fn maybe_start_server<T, F>(
        address: Option<SocketAddr>,
        mut start: F,
    ) -> Result<Option<T>, Error>
    where
        F: FnMut(&SocketAddr) -> Result<T, Error>,
    {
        address
            .map(|mut address| {
                start(&address).or_else(|e| match e {
                    Error::Io(e) => match e.kind() {
                        io::ErrorKind::AddrInUse | io::ErrorKind::PermissionDenied => {
                            warn!(
                                "Unable to bind RPC server to {}. Trying random port.",
                                address
                            );
                            address.set_port(0);
                            start(&address)
                        }
                        _ => Err(e.into()),
                    },
                    e => Err(e),
                })
            })
            .transpose()
    }

    fn deny_unsafe(addr: &SocketAddr, methods: &RpcMethods) -> rpc::DenyUnsafe {
        let is_exposed_addr = !addr.ip().is_loopback();
        match (is_exposed_addr, methods) {
            (_, RpcMethods::Unsafe) | (false, RpcMethods::Auto) => rpc::DenyUnsafe::No,
            _ => rpc::DenyUnsafe::Yes,
        }
    }

    let rpc_method_names = rpc_server::method_names(|m| gen_handler(rpc::DenyUnsafe::No, m))?;
    Ok(Box::new((
        config
            .rpc_ipc
            .as_ref()
            .map(|path| {
                rpc_server::start_ipc(
                    &*path,
                    gen_handler(
                        rpc::DenyUnsafe::No,
                        rpc_server::RpcMiddleware::new(
                            rpc_metrics.clone(),
                            rpc_method_names.clone(),
                            "ipc",
                        ),
                    )?,
                    server_metrics.clone(),
                )
                .map_err(Error::from)
            })
            .transpose()?,
        maybe_start_server(config.rpc_http, |address| {
            rpc_server::start_http(
                address,
                config.rpc_cors.as_ref(),
                gen_handler(
                    deny_unsafe(&address, &config.rpc_methods),
                    rpc_server::RpcMiddleware::new(
                        rpc_metrics.clone(),
                        rpc_method_names.clone(),
                        "http",
                    ),
                )?,
                config.rpc_max_payload,
                config.tokio_handle.clone(),
            )
            .map_err(Error::from)
        })?
        .map(|s| waiting::HttpServer(Some(s))),
        maybe_start_server(config.rpc_ws, |address| {
            rpc_server::start_ws(
                address,
                config.rpc_ws_max_connections,
                config.rpc_cors.as_ref(),
                gen_handler(
                    deny_unsafe(&address, &config.rpc_methods),
                    rpc_server::RpcMiddleware::new(
                        rpc_metrics.clone(),
                        rpc_method_names.clone(),
                        "ws",
                    ),
                )?,
                config.rpc_max_payload,
                config.ws_max_out_buffer_capacity,
                server_metrics.clone(),
                config.tokio_handle.clone(),
            )
            .map_err(Error::from)
        })?
        .map(|s| waiting::WsServer(Some(s))),
    )))
}
/***********************************************
 Spawn the tasks that are required to run a node.
************************************************/
pub fn spawn_tasks<TRpc>(params: SpawnTasksParams<TRpc>) -> Result<RpcHandlers, Error>
where
    TRpc: rpc_service::RpcExtension<rpc_service::Metadata>,
{
    let SpawnTasksParams {
        mut config,
        task_manager,
        rpc_extensions_builder,
        system_rpc_tx,
    } = params;

    let spawn_handle = task_manager.spawn_handle();

    // Prometheus metrics.
    let metrics_service =
        if let Some(PrometheusConfig { port, registry }) = config.prometheus_config.clone() {
            // Set static metrics.
            let metrics = MetricsService::with_prometheus(telemetry.clone(), &registry, &config)?;
            spawn_handle.spawn(
                "prometheus-endpoint",
                None,
                prometheus_endpoint::init_prometheus(port, registry).map(drop),
            );

            metrics
        } else {
            MetricsService::new(telemetry.clone())
        };

    // RPC
    let gen_handler = |deny_unsafe: rpc::DenyUnsafe, rpc_middleware: server::RpcMiddleware| {
        gen_handler(
            deny_unsafe,
            rpc_middleware,
            &config,
            task_manager.spawn_handle(),
            client.clone(),
            transaction_pool.clone(),
            keystore.clone(),
            &*rpc_extensions_builder,
            backend.offchain_storage(),
            system_rpc_tx.clone(),
        )
    };
    let rpc_metrics = rpc_server::RpcMetrics::new(config.prometheus_registry())?;
    let server_metrics = rpc_server::ServerMetrics::new(config.prometheus_registry())?;
    let rpc = start_rpc_servers(&config, gen_handler, rpc_metrics.clone(), server_metrics)?;
    // This is used internally, so don't restrict access to unsafe RPC
    let known_rpc_method_names = rpc_server::method_names(|m| gen_handler(rpc::DenyUnsafe::No, m))?;
    let rpc_handlers = RpcHandlers(Arc::new(
        gen_handler(
            rpc::DenyUnsafe::No,
            rpc_server::RpcMiddleware::new(rpc_metrics, known_rpc_method_names, "inbrowser"),
        )?
        .into(),
    ));

    task_manager.keep_alive((config.base_path, rpc, rpc_handlers.clone()));

    Ok(rpc_handlers)
}
