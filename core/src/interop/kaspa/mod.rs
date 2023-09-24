use std::time::Duration;

use crate::imports::*;
use crate::interop::Service;
pub use futures::{future::FutureExt, select, Future};
#[allow(unused_imports)]
use kaspa_wallet_core::rpc::{NotificationMode, Rpc, RpcCtl, WrpcEncoding};
use kaspa_wallet_core::{ConnectOptions, ConnectStrategy};

cfg_if! {
    if #[cfg(not(target_arch = "wasm32"))] {
        use std::path::PathBuf;

        pub mod config;
        pub use config::Config;
        pub mod daemon;
        pub mod inproc;
        pub use kaspad_lib::args::Args;

        pub trait Kaspad {
            fn start(&self, args: Args) -> Result<()>;
            fn stop(&self) -> Result<()>;
        }

        #[derive(Debug)]
        pub enum KaspadServiceEvents {
            StartInternalInProc { config: Config, network : Network },
            StartInternalAsDaemon { config: Config, network : Network },
            StartExternalAsDaemon { path: PathBuf, config: Config, network : Network },
            StartRemoteConnection { rpc_config : RpcConfig, network : Network },
            Exit,
        }

    } else {

        #[derive(Debug)]
        pub enum KaspadServiceEvents {
            StartRemoteConnection { rpc_config : RpcConfig, network : Network },
            Exit,
        }

    }
}

pub struct KaspaService {
    pub application_events: interop::Channel<Events>,
    pub service_events: Channel<KaspadServiceEvents>,
    pub task_ctl: Channel<()>,
    pub network: Mutex<Network>,
    pub wallet: Arc<runtime::Wallet>,
    // pub rpc : Mutex<Rpc>,
    #[cfg(not(target_arch = "wasm32"))]
    pub kaspad: Mutex<Option<Arc<dyn Kaspad + Send + Sync + 'static>>>,
}

impl KaspaService {
    pub fn new(application_events: interop::Channel<Events>, settings: &Settings) -> Self {
        // --
        // create wallet instance
        let storage = runtime::Wallet::local_store().unwrap_or_else(|e| {
            panic!("Failed to open local store: {}", e);
        });

        let wallet = runtime::Wallet::try_with_rpc(None, storage, Some(settings.node.network.into()))
            .unwrap_or_else(|e| {
                panic!("Failed to create wallet instance: {}", e);
            });
        // --
        // let wrpc_client = Arc::new(KaspaRpcClient::new_with_args(
        //     WrpcEncoding::Borsh,
        //     NotificationMode::MultiListeners,
        //     &settings.url,
        // ).expect("Unable to create KaspaService RPC client"));
        // let wrpc_ctl = wrpc_client.ctl().clone();
        // let wrpc_api: Arc<DynRpcApi> = wrpc_client;
        // let wrpc = Rpc::new(wrpc_api, wrpc_ctl);
        // --

        // let rpc = Self::create_rpc_client(&settings.rpc).expect("Kaspad Service - unable to create wRPC client");

        // create service event channel
        let service_events = Channel::unbounded();

        // enqueue startup event to the service channel to
        // start kaspad or initiate connection to remote kaspad
        match KaspadServiceEvents::try_from(&settings.node) {
            Ok(event) => {
                service_events.sender.try_send(event).unwrap_or_else(|err| {
                    println!("KaspadService error: {}", err);
                });
            }
            Err(err) => {
                println!("KaspadServiceEvents::try_from() error: {}", err);
            }
        }

        Self {
            application_events,
            service_events,
            task_ctl: Channel::oneshot(),
            network: Mutex::new(settings.node.network),
            wallet: Arc::new(wallet),
            // rpc : Mutex::new(rpc),
            // wrpc,
            #[cfg(not(target_arch = "wasm32"))]
            kaspad: Mutex::new(None),
        }
    }

    pub fn create_rpc_client(config: &RpcConfig, network: Network) -> Result<Rpc> {
        match config {
            RpcConfig::Wrpc { url, encoding } => {
                log_warning!("create_rpc_client - RPC URL: {}", url);

                let url = KaspaRpcClient::parse_url(
                    Some(url.clone()),
                    *encoding,
                    NetworkId::from(network).into(),
                )?
                .ok_or(Error::InvalidUrl(url.clone()))?;

                let wrpc_client = Arc::new(KaspaRpcClient::new_with_args(
                    *encoding,
                    NotificationMode::MultiListeners,
                    url.as_str(),
                )?);
                let rpc_ctl = wrpc_client.ctl().clone();
                let rpc_api: Arc<DynRpcApi> = wrpc_client;
                Ok(Rpc::new(rpc_api, rpc_ctl))
            }
            RpcConfig::Grpc { url: _ } => {
                unimplemented!("GPRC is not currently supported")
            }
        }
    }

    pub fn wallet(&self) -> &Arc<runtime::Wallet> {
        &self.wallet
    }

    // pub async fn stop_wallet(&self) -> Result<()> {
    //     self.wallet().stop().await.expect("Unable to stop wallet");
    //     self.wallet().bind_rpc(None).await?;
    //     Ok(())
    // }

    pub async fn stop_all_services(&self) -> Result<()> {

        if !self.wallet().has_rpc() {
            return Ok(())
        }

        if let Ok(wrpc_client) = self.wallet().rpc_api().clone().downcast_arc::<KaspaRpcClient>() {
            wrpc_client.disconnect().await?;
        } else {
            self.wallet().rpc_ctl().signal_close().await?;
        }

        self.wallet().stop().await.expect("Unable to stop wallet");
        self.wallet().bind_rpc(None).await?;

        #[cfg(not(target_arch = "wasm32"))]
        if let Some(kaspad) = self.kaspad.lock().unwrap().take() {
            println!("*** STOPPING KASPAD ***");
            if let Err(err) = kaspad.stop() {
                println!("error stopping kaspad: {}", err);
            }
            println!("*** KASPAD STOPPED ***");
        }
        Ok(())
    }

    pub async fn start_wallet_service(&self, rpc : Rpc, network : Network) -> Result<()> {
        self.wallet().set_network_id(network.into()).expect("Can not change network id while the wallet is connected");
        self.wallet().bind_rpc(Some(rpc)).await.unwrap();
        self.wallet().start().await.expect("Unable to stop wallet");
        Ok(())
    }

    pub fn update_services(&self, node_settings : &NodeSettings) {
        match KaspadServiceEvents::try_from(node_settings) {
            Ok(event) => {
                self.service_events.sender.try_send(event).unwrap_or_else(|err| {
                    println!("KaspadService error: {}", err);
                });
            }
            Err(err) => {
                println!("KaspadServiceEvents::try_from() error: {}", err);
            }
        }

    }

    
}

#[async_trait]
impl Service for KaspaService {
    async fn spawn(self: Arc<Self>) -> Result<()> {
        println!("kaspad manager service starting...");
        let this = self.clone();

        // println!("starting wallet...");
        // this.wallet.start().await.unwrap_or_else(|err| {
        //     println!("Wallet start error: {:?}", err);
        // });

        let wallet_events = this.wallet.multiplexer().channel();

        let _application_events_sender = self.application_events.sender.clone();
        // spawn(async move {
        loop {
            println!("loop...");
            select! {

                msg = wallet_events.recv().fuse() => {
                    if let Ok(event) = msg {
                        println!("wallet event: {:?}", event);
                        this.application_events.sender.send(crate::events::Events::Wallet{event}).await.unwrap();
                    } else {
                        break;
                    }
                }

                msg = this.as_ref().service_events.receiver.recv().fuse() => {

                    if let Ok(event) = msg {

                        println!("NODE EVENT: {:#?}", event);

                        match event {

                            #[cfg(not(target_arch = "wasm32"))]
                            KaspadServiceEvents::StartInternalInProc { config, network } => {

                                this.stop_all_services().await?;

                                // this.wallet().stop().await.expect("Unable to stop wallet");
                                // this.wallet().bind_rpc(None).await?;

                                // if let Some(kaspad) = self.kaspad.lock().unwrap().take() {
                                //     println!("*** STOPPING KASPAD ***");
                                //     if let Err(err) = kaspad.stop() {
                                //         println!("error stopping kaspad: {}", err);
                                //     }
                                // }

                                println!("*** STARTING NEW KASPAD ***");

                                println!("*** STARTING NEW KASPAD ***");
                                let kaspad = Arc::new(inproc::InProc::default());
                                this.kaspad.lock().unwrap().replace(kaspad.clone());
                                kaspad.start(config.into()).unwrap();


                                println!("*** SETTING UP DIRECT RPC BINDING ***");
                                let rpc_api = kaspad.rpc_core_services().expect("Unable to obtain inproc rpc api");
                                let rpc_ctl = RpcCtl::new();
                                let rpc = Rpc::new(rpc_api, rpc_ctl.clone());


                                // - CONNECT NEVER REACHES BECAUSE WHEN IT IS BROADCASTED,
                                // - MULTIPLEXER CLIENT DOES NOT YET EXISTS

                                // this.wallet().bind_rpc(Some(rpc)).await.unwrap();
                                // this.wallet().start().await.expect("Unable to stop wallet");

                                this.start_wallet_service(rpc, network).await?;

                                rpc_ctl.signal_open().await?;//.expect("Unable to signal `open` event to rpc ctl");

                            },
                            #[cfg(not(target_arch = "wasm32"))]
                            KaspadServiceEvents::StartInternalAsDaemon { config:_, network:_ } => {
                                self.stop_all_services().await?;
                                // todo!()
                            },
                            #[cfg(not(target_arch = "wasm32"))]
                            KaspadServiceEvents::StartExternalAsDaemon { path:_, config:_, network:_ } => {
                                self.stop_all_services().await?;
                                // todo!()
                            },
                            KaspadServiceEvents::StartRemoteConnection { rpc_config, network } => {

                                self.stop_all_services().await?;

                                let rpc = Self::create_rpc_client(&rpc_config, network).expect("Kaspad Service - unable to create wRPC client");
                                // let rpc = Self::create_rpc_client(&settings.rpc).expect("Kaspad Service - unable to create wRPC client");
                                let wrpc_client = rpc.rpc_api().clone().downcast_arc::<KaspaRpcClient>().ok();

                                // this.wallet().set_network_id(network.into()).expect("Can not change network id while the wallet is connected");
                                // this.wallet().bind_rpc(Some(rpc)).await.unwrap();
                                // this.wallet().start().await.expect("Unable to stop wallet");

                                this.start_wallet_service(rpc, network).await?;

                                // rpc.connect()

                                if let Some(wrpc_client) = wrpc_client {
                                    let options = ConnectOptions {
                                        block_async_connect: false,
                                        strategy: ConnectStrategy::Retry,
                                        url : None,
                                        connect_timeout: None,
                                        retry_interval: Some(Duration::from_millis(3000)),
                                    };
                                    wrpc_client.connect(options).await?;
                                }

                            },

                            KaspadServiceEvents::Exit => {
                                break;
                            }
                        }
                    } else {
                        break;
                    }
                }
            }
        }


        println!("shutting down node manager...");
        this.stop_all_services().await?;

        // this.wallet().stop().await.expect("Unable to stop wallet");
        // this.wallet().bind_rpc(None).await?;

        // #[cfg(not(target_arch = "wasm32"))]
        // if let Some(kaspad) = self.kaspad.lock().unwrap().take() {
        //     println!("shutting down kaspad manager service...");
        //     if let Err(err) = kaspad.stop() {
        //         println!("error stopping kaspad: {}", err);
        //     }
        // }

        this.task_ctl.send(()).await.unwrap();
        // Ok(())
        // });

        Ok(())
    }

    fn terminate(self: Arc<Self>) {
        println!("POSTING TERMINATION EVENT...");
        self.service_events
            .sender
            .try_send(KaspadServiceEvents::Exit)
            .unwrap();
    }

    async fn join(self: Arc<Self>) -> Result<()> {
        self.task_ctl.recv().await.unwrap();
        Ok(())
    }
}

impl TryFrom<&NodeSettings> for KaspadServiceEvents {
    type Error = Error;
    fn try_from(node_settings: &NodeSettings) -> std::result::Result<Self, Self::Error> {
        cfg_if! {
            if #[cfg(not(target_arch = "wasm32"))] {


                match &node_settings.kaspad {
                    KaspadNodeKind::IntegratedInProc => {
                        // let config = ;
                        Ok(KaspadServiceEvents::StartInternalInProc { config : Config::from(node_settings.clone()), network : node_settings.network })
                    }
                    KaspadNodeKind::IntegratedAsDaemon => {
                        Ok(KaspadServiceEvents::StartInternalAsDaemon { config : Config::from(node_settings.clone()), network : node_settings.network })
                    }
                    KaspadNodeKind::ExternalAsDaemon => {
                        let path = node_settings.kaspad_node_binary.clone().ok_or(Error::MissingExternalKaspadBinary)?;
                        Ok(KaspadServiceEvents::StartExternalAsDaemon { path : PathBuf::from(path), config : Config::from(node_settings.clone()), network : node_settings.network })
                    }
                    KaspadNodeKind::Remote => {
                        Ok(KaspadServiceEvents::StartRemoteConnection { rpc_config : node_settings.into(), network : node_settings.network })
                    }
                }

            } else {

                match &node_settings.kaspad {
                    KaspadNodeKind::Remote => {
                        Ok(KaspadServiceEvents::StartRemoteConnection { rpc_config : node_settings.into(), network : node_settings.network })
                    }
                }
            }
        }
    }
}