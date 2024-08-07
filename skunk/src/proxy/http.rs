use std::{
    convert::Infallible,
    net::SocketAddr,
    sync::Arc,
};

use http_body_util::Empty;
use hyper::{
    body::Incoming,
    service::service_fn,
    Method,
    Request,
    Response,
    StatusCode,
};
use hyper_util::rt::TokioIo;
use tokio::net::{
    TcpListener,
    TcpStream,
};
use tokio_util::sync::CancellationToken;
use tracing::Instrument;

use crate::{
    connect::{
        Connect,
        ConnectTcp,
    },
    layer::{
        Layer,
        Passthrough,
    },
    util::error::ResultExt,
};

pub const DEFAULT_PORT: u16 = 8080;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error")]
    Io(#[from] std::io::Error),

    #[error("hyper error")]
    Hyper(#[from] hyper::Error),
}

pub struct Builder<C, L> {
    bind_address: SocketAddr,
    shutdown: CancellationToken,
    #[cfg(feature = "tls")]
    tls_client_config: Option<Arc<rustls::ClientConfig>>,
    connect: C,
    layer: L,
}

impl Default for Builder<ConnectTcp, Passthrough> {
    fn default() -> Self {
        Self {
            bind_address: ([127, 0, 0, 1], DEFAULT_PORT).into(),
            shutdown: Default::default(),
            #[cfg(feature = "tls")]
            tls_client_config: None,
            connect: ConnectTcp,
            layer: Passthrough,
        }
    }
}

impl<C, L> Builder<C, L> {
    pub fn with_bind_address(mut self, bind_address: impl Into<SocketAddr>) -> Self {
        self.bind_address = bind_address.into();
        self
    }

    pub fn with_graceful_shutdown(mut self, shutdown: CancellationToken) -> Self {
        self.shutdown = shutdown;
        self
    }

    #[cfg(feature = "tls")]
    pub fn with_tls_client(
        mut self,
        tls_client_config: impl Into<Arc<rustls::ClientConfig>>,
    ) -> Self {
        self.tls_client_config = Some(tls_client_config.into());
        self
    }

    pub fn with_connect<D>(self, connect: D) -> Builder<D, L> {
        Builder {
            bind_address: self.bind_address,
            shutdown: self.shutdown,
            #[cfg(feature = "tls")]
            tls_client_config: self.tls_client_config,
            connect,
            layer: self.layer,
        }
    }

    pub fn with_layer<M>(self, layer: M) -> Builder<C, M> {
        Builder {
            bind_address: self.bind_address,
            shutdown: self.shutdown,
            #[cfg(feature = "tls")]
            tls_client_config: self.tls_client_config,
            connect: self.connect,
            layer,
        }
    }

    pub async fn serve(self) -> Result<(), Error>
    where
        C: Connect + Clone + Send + 'static,
        L: for<'s, 't> Layer<&'s mut (), &'t mut <C as Connect>::Connection>
            + Clone
            + Send
            + 'static,
    {
        run(self.bind_address, self.shutdown, self.connect, self.layer).await?;
        Ok(())
    }
}

async fn run<C, L>(
    bind_address: SocketAddr,
    shutdown: CancellationToken,
    connect: C,
    layer: L,
) -> Result<(), Error>
where
    C: Connect + Clone + Send + 'static,
    L: for<'s, 't> Layer<&'s mut (), &'t mut <C as Connect>::Connection> + Clone + Send + 'static,
{
    let listener = TcpListener::bind(bind_address).await?;

    loop {
        tokio::select! {
            result = listener.accept() => {
                let (connection, address) = result?;
                let shutdown = shutdown.clone();
                let connect = connect.clone();
                let layer = layer.clone();
                let span = tracing::info_span!("http-proxy", ?address);

                tokio::spawn(async move {
                    tokio::select!{
                        result = handle_connection(connection, connect, layer) => {
                            let _ = result.log_error();
                        },
                        _ = shutdown.cancelled() => {},
                    }
                }.instrument(span));
            },
            _ = shutdown.cancelled() => {},
        }
    }
}

async fn handle_connection<C, L>(connection: TcpStream, connect: C, layer: L) -> Result<(), Error>
where
    C: Connect + Clone,
    L: for<'s, 't> Layer<&'s mut (), &'t mut <C as Connect>::Connection> + Clone,
{
    hyper::server::conn::http1::Builder::new()
        .serve_connection(
            TokioIo::new(connection),
            service_fn(move |request: Request<Incoming>| {
                let _connect = connect.clone();
                let _layer = layer.clone();
                async move {
                    match request.method() {
                        &Method::CONNECT => {
                            tokio::spawn(async move {
                                let _upgraded = hyper::upgrade::on(request).await?;
                                Ok::<(), Error>(())
                            });

                            let response = Response::builder()
                                .status(StatusCode::SWITCHING_PROTOCOLS)
                                .body(Empty::<&[u8]>::new())
                                .expect("build invalid response");

                            Ok::<_, Infallible>(response)
                        }
                        _ => {
                            todo!();
                        }
                    }
                }
            }),
        )
        .await?;
    Ok(())
}
