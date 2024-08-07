use std::{
    collections::HashSet,
    sync::Arc,
};

use axum::Router;
use color_eyre::eyre::Error;
use skunk::{
    address::TcpAddress,
    connect::{
        Connect,
        ConnectTcp,
    },
    protocol::{
        http,
        tls,
    },
    proxy::{
        pcap::{
            self,
            interface::Interface,
            VirtualNetwork,
        },
        socks::server as socks,
        DestinationAddress,
        Passthrough,
        Proxy,
    },
};
use skunk_util::error::ResultExt;
use tokio::{
    net::TcpStream,
    task::JoinSet,
};
use tokio_util::sync::CancellationToken;
use tracing::Instrument;

use crate::{
    env::{
        args::ProxyArgs,
        Environment,
    },
    util::{
        serve_ui::ServeUi,
        shutdown::cancel_on_ctrlc_or_sigterm,
    },
};

pub async fn run(environment: Environment, args: ProxyArgs) -> Result<(), Error> {
    let pcap_interface = if args.pcap.enabled {
        fn print_interfaces() -> Result<(), Error> {
            println!("Available interfaces:");
            for interface in Interface::list()? {
                println!("{}\n", interface.name());
            }
            Ok(())
        }

        if let Some(interface) = args.pcap.interface {
            let interface_opt = Interface::from_name(&interface)?;
            if interface_opt.is_none() {
                eprintln!("Interface '{interface}' not found");
                print_interfaces()?;
                return Ok(());
            }
            interface_opt
        }
        else {
            print_interfaces()?;
            return Ok(());
        }
    }
    else {
        None
    };

    // create TLS context
    let tls = environment.tls_context().await?;

    // target filters
    let filter = Arc::new(if args.filter.is_empty() {
        tracing::info!("Matching all flows");
        Filter::All
    }
    else {
        tracing::info!("Matching: {:?}", args.filter);
        Filter::Set(args.filter.into_iter().collect())
    });

    // shutdown token
    let shutdown = if args.no_graceful_shutdown {
        CancellationToken::default()
    }
    else {
        cancel_on_ctrlc_or_sigterm()
    };

    let mut join_set = JoinSet::new();

    if args.socks.enabled {
        let shutdown = shutdown.clone();

        join_set.spawn(async move {
            // run the SOCKS server. `proxy` will handle connections. The default
            // [`Connect`][skunk::connect::Connect] (i.e.
            // [`ConnectTcp`][skunk::connect::ConnectTcp]) is used.
            let mut listener = args.socks.builder()?.listen().await?;
            tracing::info!("SOCKS server listening on: {}", args.socks.bind_address);

            let mut join_set = JoinSet::default();

            loop {
                let request = tokio::select! {
                    _ = shutdown.cancelled() => break,
                    request_res = listener.next() => request_res?,
                };

                match ConnectTcp.connect(request.destination_address()).await {
                    Ok(outgoing) => {
                        let bind_address = outgoing.local_addr().unwrap().into();
                        let incoming = request.accept(bind_address).await?;
                        let tls = tls.clone();
                        let filter = filter.clone();
                        let shutdown = shutdown.clone();

                        join_set.spawn(async move {
                            tokio::select! {
                                _ = shutdown.cancelled() => {},
                                result = proxy(tls, filter, incoming, outgoing) => {
                                    let _ = result.log_error();
                                }
                            }
                        });
                    }
                    Err(_) => {
                        request.reject(None);
                    }
                }
            }

            while join_set.join_next().await.is_some() {}

            Ok::<(), Error>(())
        });
    }

    if let Some(interface) = pcap_interface {
        join_set.spawn({
            let shutdown = shutdown.clone();
            let interface = interface.clone();
            async move {
                let _hostapd = if args.pcap.ap {
                    let country_code = std::env::var("HOSTAPD_CC")
                    .expect("Environment variable `HOSTAPD_CC` not set. You need to set this variable to your country code.");

                    tracing::info!("Starting hostapd");
                    let mut hostapd = pcap::ap::Builder::new(&interface, &country_code)
                            .with_channel(11)
                            .start()?;

                    tracing::info!("Waiting for hostapd to configure the interface...");
                    hostapd.ready().await?;
                    tracing::info!("hostapd ready");

                    Some(hostapd)
                }
                else {
                    None
                };

                let _network = VirtualNetwork::new(&interface)?;
                shutdown.cancelled().await;
                Ok::<(), Error>(())
            }
        });
    }

    if args.api.enabled {
        let shutdown = shutdown.clone();
        let mut api_builder = super::api::builder(environment.clone());
        let serve_ui = ServeUi::from_environment(&environment, &mut api_builder).await?;

        join_set.spawn(async move {
            tracing::info!(bind_address = ?args.api.bind_address, "Starting API");

            let router = Router::new()
                .nest("/api", api_builder.finish())
                .fallback_service(serve_ui);

            let listener = tokio::net::TcpListener::bind(args.api.bind_address).await?;
            tracing::info!(bind_address = ?args.api.bind_address, "UI and API being served at: http://{}", args.api.bind_address);

            axum::serve(listener, router)
                .with_graceful_shutdown(shutdown.cancelled_owned())
                .await?;

            Ok::<(), Error>(())
        });
    }

    // join all tasks
    while let Some(result) = join_set.join_next().await {
        let _ = result.log_error();
    }

    Ok(())
}

/// Proxy connections.
///
/// This will first check if the connection matches any filters. Then it will
/// decide using the port whether to decrypt TLS for that connection. Finally it
/// will run a HTTP server and client to proxy HTTP requests.
async fn proxy(
    tls: tls::Context,
    filter: Arc<Filter>,
    incoming: socks::Incoming,
    outgoing: TcpStream,
) -> Result<(), skunk::Error> {
    let destination_address = incoming.destination_address();

    if filter.matches(destination_address) {
        let span = tracing::info_span!("connection", destination = %destination_address);

        let is_tls = destination_address.port == 443;
        let (incoming, outgoing) = tls.maybe_decrypt(incoming, outgoing, is_tls).await?;

        http::proxy(incoming, outgoing, |request, send_request| {
            let span = tracing::info_span!(
                parent: &span,
                "request",
                method = %request.method(),
                uri = %request.uri()
            );

            async move {
                // log request
                tracing::info!("Request");

                let response = send_request.send(request).await?;

                // log response
                tracing::info!(
                    status = %response.status(),
                    "Response"
                );

                Ok(response)
            }
            .instrument(span)
        })
        .await?;
    }
    else {
        Passthrough.proxy(incoming, outgoing).await?;
    };

    Ok::<_, skunk::Error>(())
}

/// A simple filter to decide which target addresses should be intercepted.
#[derive(Clone, Debug)]
enum Filter {
    All,
    Set(HashSet<TcpAddress>),
}

impl Filter {
    pub fn matches(&self, address: &TcpAddress) -> bool {
        if address.port != 80 && address.port != 443 {
            return false;
        }

        match self {
            Filter::All => true,
            Filter::Set(targets) => targets.contains(address),
        }
    }
}
