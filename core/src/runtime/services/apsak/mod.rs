use crate::imports::*;
use crate::runtime::Service;
pub use futures::{future::FutureExt, select, Future};
use apsak_wallet_core::api::*;
use apsak_wallet_core::events::Events as CoreWalletEvents;
#[allow(unused_imports)]
use apsak_wallet_core::rpc::{
    ConnectOptions, ConnectStrategy, NotificationMode, Rpc, RpcCtl, WrpcEncoding,
};
use workflow_core::runtime;

const ENABLE_PREEMPTIVE_DISCONNECT: bool = true;

cfg_if! {
    if #[cfg(not(target_arch = "wasm32"))] {
        #[cfg(not(target_arch = "wasm32"))]
        use apsak_rpc_service::service::RpcCoreService;

        const LOG_BUFFER_LINES: usize = 4096;
        const LOG_BUFFER_MARGIN: usize = 128;
    }
}

cfg_if! {
    if #[cfg(not(target_arch = "wasm32"))] {
        use std::path::PathBuf;

        pub mod config;
        pub use config::Config;
        pub mod daemon;
        pub mod inproc;
        pub mod logs;
        use logs::Log;
        pub use apsakd_lib::args::Args;

        #[async_trait]
        pub trait Apsakd {
            async fn start(self : Arc<Self>, config : Config) -> Result<()>;
            async fn stop(self : Arc<Self>) -> Result<()>;
        }

        #[derive(Debug, Clone)]
        pub enum ApsakdServiceEvents {
            StartInternalInProc { config: Config, network : Network },
            StartInternalAsDaemon { config: Config, network : Network },
            StartExternalAsDaemon { path: PathBuf, config: Config, network : Network },
            StartRemoteConnection { rpc_config : RpcConfig, network : Network },
            Stdout { line : String },
            Disable { network : Network },
            Exit,
        }

        pub fn update_logs_flag() -> &'static Arc<AtomicBool> {
            static FLAG: OnceLock<Arc<AtomicBool>> = OnceLock::new();
            FLAG.get_or_init(||Arc::new(AtomicBool::new(false)))
        }

        pub fn update_metrics_flag() -> &'static Arc<AtomicBool> {
            static FLAG: OnceLock<Arc<AtomicBool>> = OnceLock::new();
            FLAG.get_or_init(||Arc::new(AtomicBool::new(false)))
        }

    } else {

        #[derive(Debug)]
        pub enum ApsakdServiceEvents {
            StartRemoteConnection { rpc_config : RpcConfig, network : Network },
            Disable { network : Network },
            Exit,
        }

    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct Context {}

pub struct ApsakService {
    pub application_events: ApplicationEventsChannel,
    pub service_events: Channel<ApsakdServiceEvents>,
    pub task_ctl: Channel<()>,
    pub network: Mutex<Network>,
    pub wallet: Arc<dyn WalletApi>,
    pub services_start_instant: Mutex<Option<Instant>>,
    #[cfg(not(target_arch = "wasm32"))]
    pub apsakd: Mutex<Option<Arc<dyn Apsakd + Send + Sync + 'static>>>,
    #[cfg(not(target_arch = "wasm32"))]
    pub logs: Mutex<Vec<Log>>,
    pub connect_on_startup: Option<NodeSettings>,
}

impl ApsakService {
    pub fn new(
        application_events: ApplicationEventsChannel,
        settings: &Settings,
        wallet: Option<Arc<dyn WalletApi>>,
    ) -> Self {
        // --
        // create wallet instance
        let storage = CoreWallet::local_store().unwrap_or_else(|e| {
            panic!("Failed to open local store: {}", e);
        });

        let wallet = wallet.unwrap_or_else(|| {
            Arc::new(
                CoreWallet::try_with_rpc(None, storage, Some(settings.node.network.into()))
                    .unwrap_or_else(|e| {
                        panic!("Failed to create wallet instance: {}", e);
                    }),
            )
        });

        // create service event channel
        let service_events = Channel::unbounded();

        if !settings.initialized {
            log_warn!("ApsakService::new(): Settings are not initialized");
        }

        Self {
            connect_on_startup: settings.initialized.then(|| settings.node.clone()),
            application_events,
            service_events,
            task_ctl: Channel::oneshot(),
            network: Mutex::new(settings.node.network),
            wallet,
            services_start_instant: Mutex::new(None),
            #[cfg(not(target_arch = "wasm32"))]
            apsakd: Mutex::new(None),
            #[cfg(not(target_arch = "wasm32"))]
            logs: Mutex::new(Vec::new()),
        }
    }

    pub async fn apply_node_settings(&self, node_settings: &NodeSettings) -> Result<()> {
        match ApsakdServiceEvents::from_node_settings(node_settings, None) {
            Ok(event) => {
                // log_trace!("ApsakService::new(): emitting startup event: {:?}", event);
                self.service_events
                    .sender
                    .try_send(event)
                    .unwrap_or_else(|err| {
                        log_error!("ApsakdService error: {}", err);
                    });
            }
            Err(err) => {
                log_error!("ApsakdServiceEvents::try_from() error: {}", err);
            }
        }
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn retain(&self, apsakd: Arc<dyn Apsakd + Send + Sync + 'static>) {
        self.apsakd.lock().unwrap().replace(apsakd);
    }

    pub fn create_rpc_client(config: &RpcConfig, network: Network) -> Result<Rpc> {
        match config {
            RpcConfig::Wrpc { url, encoding } => {
                // log_warn!("create_rpc_client - RPC URL: {:?}", url);
                let url = url.clone().unwrap_or_else(|| "127.0.0.1".to_string());
                let url =
                    ApsakRpcClient::parse_url(url, *encoding, NetworkId::from(network).into())?;

                let wrpc_client = Arc::new(ApsakRpcClient::new_with_args(
                    *encoding,
                    Some(url.as_str()),
                    // TODO: introduce resolver for public node resolution
                    None,
                    None,
                    None,
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

    pub async fn connect_rpc_client(&self) -> Result<()> {
        if let Some(wallet) = self.core_wallet() {
            if let Ok(wrpc_client) = wallet.rpc_api().clone().downcast_arc::<ApsakRpcClient>() {
                let options = ConnectOptions {
                    block_async_connect: false,
                    strategy: ConnectStrategy::Retry,
                    url: None,
                    connect_timeout: None,
                    retry_interval: Some(Duration::from_millis(3000)),
                };
                wrpc_client.connect(Some(options)).await?;
            } else {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    if wallet
                        .rpc_api()
                        .clone()
                        .downcast_arc::<RpcCoreService>()
                        .is_ok()
                    {
                        wallet.rpc_ctl().signal_open().await?;
                    } else {
                        unimplemented!("connect_rpc_client(): RPC client is not supported")
                    }
                }
            }
        }
        Ok(())
    }

    pub fn wallet(&self) -> Arc<dyn WalletApi> {
        self.wallet.clone()
    }

    pub fn core_wallet(&self) -> Option<Arc<CoreWallet>> {
        self.wallet.clone().downcast_arc::<CoreWallet>().ok()
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn logs(&self) -> MutexGuard<'_, Vec<Log>> {
        self.logs.lock().unwrap()
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub async fn update_logs(&self, line: String) {
        {
            let mut logs = self.logs.lock().unwrap();
            if logs.len() > LOG_BUFFER_LINES {
                logs.drain(0..LOG_BUFFER_MARGIN);
            }
            logs.push(line.as_str().into());
        }

        if update_logs_flag().load(Ordering::SeqCst) {
            self.application_events
                .sender
                .send(crate::events::Events::UpdateLogs)
                .await
                .unwrap();
        }
    }

    pub fn rpc_url(&self) -> Option<String> {
        if let Some(wallet) = self.core_wallet() {
            if !wallet.has_rpc() {
                None
            } else if let Ok(wrpc_client) =
                wallet.rpc_api().clone().downcast_arc::<ApsakRpcClient>()
            {
                wrpc_client.url()
            } else {
                None
            }
        } else {
            None
        }
    }

    fn is_wrpc_client(&self) -> bool {
        if let Some(wallet) = self.core_wallet() {
            wallet.has_rpc()
                && wallet
                    .rpc_api()
                    .clone()
                    .downcast_arc::<ApsakRpcClient>()
                    .is_ok()
        } else {
            false
        }
    }

    async fn disconnect_rpc(&self) -> Result<()> {
        if let Some(wallet) = self.core_wallet() {
            if let Ok(wrpc_client) = wallet.rpc_api().clone().downcast_arc::<ApsakRpcClient>() {
                wrpc_client.disconnect().await?;
            } else {
                wallet.rpc_ctl().signal_close().await?;
            }
        }
        Ok(())
    }

    pub async fn stop_all_services(&self) -> Result<()> {
        self.services_start_instant.lock().unwrap().take();

        if let Some(wallet) = self.core_wallet() {
            if !wallet.has_rpc() {
                return Ok(());
            }

            let preemptive_disconnect = ENABLE_PREEMPTIVE_DISCONNECT && self.is_wrpc_client();

            if preemptive_disconnect {
                self.disconnect_rpc().await?;
            }

            for service in crate::runtime::runtime().services().into_iter() {
                let instant = Instant::now();
                service.clone().detach_rpc().await?;
                if instant.elapsed().as_millis() > 1_000 {
                    log_warn!(
                        "WARNING: detach_rpc() for '{}' took {} msec",
                        service.name(),
                        instant.elapsed().as_millis()
                    );
                }
            }

            if !preemptive_disconnect {
                self.disconnect_rpc().await?;
            }

            wallet.stop().await.expect("Unable to stop wallet");
            wallet.bind_rpc(None).await?;

            #[cfg(not(target_arch = "wasm32"))]
            {
                let apsakd = self.apsakd.lock().unwrap().take();
                if let Some(apsakd) = apsakd {
                    if let Err(err) = apsakd.stop().await {
                        println!("error shutting down apsakd: {}", err);
                    }
                }
            }
        } else {
            self.wallet().disconnect().await?;
        }
        Ok(())
    }

    pub async fn start_all_services(
        self: &Arc<Self>,
        rpc: Option<Rpc>,
        network: Network,
    ) -> Result<()> {
        self.services_start_instant
            .lock()
            .unwrap()
            .replace(Instant::now());

        *self.network.lock().unwrap() = network;

        if let (Some(rpc), Some(wallet)) = (rpc, self.core_wallet()) {
            let rpc_api = rpc.rpc_api().clone();

            wallet
                .set_network_id(&network.into())
                .expect("Can not change network id while the wallet is connected");

            wallet.bind_rpc(Some(rpc)).await.unwrap();
            wallet
                .start()
                .await
                .expect("Unable to start wallet service");

            for service in crate::runtime::runtime().services().into_iter() {
                service.attach_rpc(&rpc_api).await?;
            }

            Ok(())
        } else {
            self.wallet()
                .connect_call(ConnectRequest {
                    url: None,
                    network_id: network.into(),
                })
                .await?;

            Ok(())
        }
    }

    pub fn update_services(&self, node_settings: &NodeSettings, options: Option<RpcOptions>) {
        match ApsakdServiceEvents::from_node_settings(node_settings, options) {
            Ok(event) => {
                self.service_events
                    .sender
                    .try_send(event)
                    .unwrap_or_else(|err| {
                        println!("ApsakdService error: {}", err);
                    });
            }
            Err(err) => {
                println!("ApsakdServiceEvents::try_from() error: {}", err);
            }
        }
    }

    fn network(&self) -> Network {
        *self.network.lock().unwrap()
    }

    async fn handle_network_change(&self, network: Network) -> Result<()> {
        if network != self.network() {
            self.application_events
                .send(Events::NetworkChange(network))
                .await?;
        }

        Ok(())
    }

    pub async fn connect_all_services(&self) -> Result<()> {
        for service in crate::runtime::runtime().services().into_iter() {
            service.connect_rpc().await?;
        }

        Ok(())
    }

    pub async fn disconnect_all_services(&self) -> Result<()> {
        for service in crate::runtime::runtime().services().into_iter() {
            service.disconnect_rpc().await?;
        }

        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn update_storage(&self) {
        const STORAGE_UPDATE_DELAY: Duration = Duration::from_millis(3000);

        let options = StorageUpdateOptions::default()
            .if_not_present()
            .with_delay(STORAGE_UPDATE_DELAY);

        runtime().update_storage(options);
    }

    async fn handle_event(self: &Arc<Self>, event: ApsakdServiceEvents) -> Result<bool> {
        match event {
            #[cfg(not(target_arch = "wasm32"))]
            ApsakdServiceEvents::Stdout { line } => {
                let wallet = self.core_wallet().ok_or(Error::WalletIsNotLocal)?;
                if !wallet.utxo_processor().is_synced() {
                    wallet
                        .utxo_processor()
                        .sync_proc()
                        .handle_stdout(&line)
                        .await?;
                }

                self.update_logs(line).await;
            }

            #[cfg(not(target_arch = "wasm32"))]
            ApsakdServiceEvents::StartInternalInProc { config, network } => {
                self.stop_all_services().await?;

                self.handle_network_change(network).await?;

                let apsakd = Arc::new(inproc::InProc::default());
                self.retain(apsakd.clone());

                apsakd.clone().start(config).await.unwrap();

                let rpc_api = apsakd
                    .rpc_core_services()
                    .expect("Unable to obtain inproc rpc api");
                let rpc_ctl = RpcCtl::new();
                let rpc = Rpc::new(rpc_api, rpc_ctl.clone());

                self.start_all_services(Some(rpc), network).await?;
                self.connect_rpc_client().await?;

                self.update_storage();
            }
            #[cfg(not(target_arch = "wasm32"))]
            ApsakdServiceEvents::StartInternalAsDaemon { config, network } => {
                self.stop_all_services().await?;

                self.handle_network_change(network).await?;

                let apsakd = Arc::new(daemon::Daemon::new(None, &self.service_events));
                self.retain(apsakd.clone());
                apsakd.clone().start(config).await.unwrap();

                let rpc_config = RpcConfig::Wrpc {
                    url: None,
                    encoding: WrpcEncoding::Borsh,
                };

                let rpc = Self::create_rpc_client(&rpc_config, network)
                    .expect("Apsakd Service - unable to create wRPC client");
                self.start_all_services(Some(rpc), network).await?;
                self.connect_rpc_client().await?;

                self.update_storage();
            }
            #[cfg(not(target_arch = "wasm32"))]
            ApsakdServiceEvents::StartExternalAsDaemon {
                path,
                config,
                network,
            } => {
                self.stop_all_services().await?;

                self.handle_network_change(network).await?;

                let apsakd = Arc::new(daemon::Daemon::new(Some(path), &self.service_events));
                self.retain(apsakd.clone());

                apsakd.clone().start(config).await.unwrap();

                let rpc_config = RpcConfig::Wrpc {
                    url: None,
                    encoding: WrpcEncoding::Borsh,
                };

                let rpc = Self::create_rpc_client(&rpc_config, network)
                    .expect("Apsakd Service - unable to create wRPC client");
                self.start_all_services(Some(rpc), network).await?;
                self.connect_rpc_client().await?;

                self.update_storage();
            }
            ApsakdServiceEvents::StartRemoteConnection {
                rpc_config,
                network,
            } => {
                if runtime::is_chrome_extension() {
                    self.stop_all_services().await?;

                    self.handle_network_change(network).await?;
                    self.wallet().change_network_id(network.into()).await.ok();

                    self.start_all_services(None, network).await?;
                    self.connect_rpc_client().await?;
                } else {
                    self.stop_all_services().await?;

                    self.handle_network_change(network).await?;

                    let rpc = Self::create_rpc_client(&rpc_config, network)
                        .expect("Apsakd Service - unable to create wRPC client");
                    self.start_all_services(Some(rpc), network).await?;
                    self.connect_rpc_client().await?;
                }
            }

            ApsakdServiceEvents::Disable { network } => {
                if let Some(wallet) = self.core_wallet() {
                    self.stop_all_services().await?;

                    self.handle_network_change(network).await?;

                    if wallet.is_open() {
                        wallet.close().await.ok();
                    }

                    // re-apply network id to allow wallet
                    // to be opened offline in disconnected
                    // mode by changing network id in settings
                    wallet.set_network_id(&network.into()).ok();
                } else if runtime::is_chrome_extension() {
                    self.stop_all_services().await?;
                    self.wallet().wallet_close().await.ok();
                    self.handle_network_change(network).await?;
                    self.wallet().change_network_id(network.into()).await.ok();
                }
            }

            ApsakdServiceEvents::Exit => {
                return Ok(true);
            }
        }

        Ok(false)
    }

    async fn handle_multiplexer(
        &self,
        event: Box<apsak_wallet_core::events::Events>,
    ) -> Result<()> {
        // use apsak_wallet_core::events::Events as CoreWalletEvents;

        match *event {
            CoreWalletEvents::DaaScoreChange { .. } => {}
            CoreWalletEvents::Connect { .. } => {
                self.connect_all_services().await?;
            }
            CoreWalletEvents::Disconnect { .. } => {
                self.disconnect_all_services().await?;
            }
            _ => {
                // println!("wallet event: {:?}", event);
            }
        }
        self.application_events
            .sender
            .send(crate::events::Events::Wallet { event })
            .await
            .unwrap();
        // }

        Ok(())
    }

    fn core_wallet_notify(&self, event: apsak_wallet_core::events::Events) -> Result<()> {
        self.application_events
            .sender
            .try_send(crate::events::Events::Wallet {
                event: Box::new(event),
            })?;
        // .try_send(Box::new(event))?;
        Ok(())
    }

    fn notify(&self, event: crate::events::Events) -> Result<()> {
        self.application_events.sender.try_send(event)?;
        Ok(())
    }
}

#[async_trait]
impl Service for ApsakService {
    fn name(&self) -> &'static str {
        "apsak-service"
    }

    async fn spawn(self: Arc<Self>) -> Result<()> {
        let _application_events_sender = self.application_events.sender.clone();

        // ^ TODO: - CHECK IF THE WALLET IS OPEN, GET WALLET CONTEXT
        // ^ TODO: - CHECK IF THE WALLET IS OPEN, GET WALLET CONTEXT
        // ^ TODO: - CHECK IF THE WALLET IS OPEN, GET WALLET CONTEXT
        // ^ TODO: - CHECK IF THE WALLET IS OPEN, GET WALLET CONTEXT
        // ^ TODO: - CHECK IF THE WALLET IS OPEN, GET WALLET CONTEXT
        // ^ TODO: - CHECK IF THE WALLET IS OPEN, GET WALLET CONTEXT
        // ^ TODO: - CHECK IF THE WALLET IS OPEN, GET WALLET CONTEXT
        // ^ TODO: - CHECK IF THE WALLET IS OPEN, GET WALLET CONTEXT

        let status = if runtime::is_chrome_extension() {
            self.wallet().get_status(Some("apsak-ng")).await.ok()
        } else {
            None
        };

        if let Some(status) = status {
            let GetStatusResponse {
                is_connected,
                is_open: _,
                is_synced,
                url,
                is_wrpc_client: _,
                network_id,
                context,
                selected_account_id,
                wallet_descriptor,
                account_descriptors,
            } = status;

            if let Some(context) = context {
                let _context = Context::try_from_slice(&context)?;

                if is_connected {
                    let network_id = network_id.unwrap_or_else(|| self.network().into());

                    // let event = Box::new(apsak_wallet_core::events::Events::Connect {
                    //     network_id,
                    //     url: url.clone(),
                    // });
                    // self.application_events
                    //     .sender
                    //     .try_send(crate::events::Events::Wallet { event })
                    //     // .await
                    //     .unwrap();

                    self.core_wallet_notify(CoreWalletEvents::Connect {
                        network_id,
                        url: url.clone(),
                    })
                    .unwrap();

                    // ^ TODO - Get appropriate `server_version`
                    let server_version = Default::default();
                    // let event = Box::new(CoreWalletEvents::ServerStatus {
                    //     is_synced,
                    //     network_id,
                    //     url,
                    //     server_version,
                    // });
                    // self.application_events
                    //     .sender
                    //     .try_send(crate::events::Events::Wallet { event })
                    //     // .await
                    //     .unwrap();

                    self.core_wallet_notify(CoreWalletEvents::ServerStatus {
                        is_synced,
                        network_id,
                        url,
                        server_version,
                    })
                    .unwrap();
                }

                if let (Some(wallet_descriptor), Some(account_descriptors)) =
                    (wallet_descriptor, account_descriptors)
                {
                    self.core_wallet_notify(CoreWalletEvents::WalletOpen {
                        wallet_descriptor: Some(wallet_descriptor),
                        account_descriptors: Some(account_descriptors),
                    })
                    .unwrap();
                }

                if let Some(selected_account_id) = selected_account_id {
                    self.core_wallet_notify(CoreWalletEvents::AccountSelection {
                        id: Some(selected_account_id),
                    })
                    .unwrap();

                    self.notify(crate::events::Events::ChangeSection(TypeId::of::<
                        crate::modules::account_manager::AccountManager,
                    >(
                    )))
                    .unwrap();
                }

                // ^ MOVE THIS FUNCTION TO "bootstrap()"
                // ^ MOVE THIS FUNCTION TO "bootstrap()"
                // ^ MOVE THIS FUNCTION TO "bootstrap()"
            } else {
                // new instance - emit startup event
                if let Some(node_settings) = self.connect_on_startup.as_ref() {
                    self.apply_node_settings(node_settings).await?;
                }

                // new instance - setup new context
                let context = Context {};
                self.wallet()
                    .retain_context("apsak-ng", Some(context.try_to_vec()?))
                    .await?;
            }
        } else {
            // new instance - emit startup event
            if let Some(node_settings) = self.connect_on_startup.as_ref() {
                self.apply_node_settings(node_settings).await?;
            }
        }
        // else if let Some(node_settings) = self.connect_on_startup.as_ref() {
        //     // self.update_services(node_settings, None);
        //     self.apply_node_settings(node_settings).await?;
        // }

        if let Some(wallet) = self.core_wallet() {
            // wallet.multiplexer().channel()
            let wallet_events = wallet.multiplexer().channel();

            loop {
                select! {
                    msg = wallet_events.recv().fuse() => {
                    // msg = wallet.multiplexer().channel().recv().fuse() => {
                        if let Ok(event) = msg {
                            self.handle_multiplexer(event).await?;
                        } else {
                            break;
                        }
                    }

                    msg = self.as_ref().service_events.receiver.recv().fuse() => {
                        if let Ok(event) = msg {
                            if self.handle_event(event).await? {
                                break;
                            }

                        } else {
                            break;
                        }
                    }
                }
            }
        } else {
            loop {
                select! {
                    // msg = wallet_events.recv().fuse() => {
                    // // msg = wallet.multiplexer().channel().recv().fuse() => {
                    //     if let Ok(event) = msg {
                    //         self.handle_multiplexer(event).await?;
                    //     } else {
                    //         break;
                    //     }
                    // }

                    msg = self.as_ref().service_events.receiver.recv().fuse() => {
                        if let Ok(event) = msg {
                            if self.handle_event(event).await? {
                                break;
                            }

                        } else {
                            break;
                        }
                    }
                }
            }
        };

        self.stop_all_services().await?;
        self.task_ctl.send(()).await.unwrap();

        Ok(())
    }

    fn terminate(self: Arc<Self>) {
        self.service_events
            .sender
            .try_send(ApsakdServiceEvents::Exit)
            .unwrap();
    }

    async fn join(self: Arc<Self>) -> Result<()> {
        self.task_ctl.recv().await.unwrap();
        Ok(())
    }
}

impl ApsakdServiceEvents {
    pub fn from_node_settings(
        node_settings: &NodeSettings,
        options: Option<RpcOptions>,
    ) -> Result<Self> {
        cfg_if! {
            if #[cfg(not(target_arch = "wasm32"))] {

                match &node_settings.node_kind {
                    ApsakdNodeKind::Disable => {
                        Ok(ApsakdServiceEvents::Disable { network : node_settings.network })
                    }
                    ApsakdNodeKind::IntegratedInProc => {
                        // let config = ;
                        Ok(ApsakdServiceEvents::StartInternalInProc { config : Config::from(node_settings.clone()), network : node_settings.network })
                    }
                    ApsakdNodeKind::IntegratedAsDaemon => {
                        Ok(ApsakdServiceEvents::StartInternalAsDaemon { config : Config::from(node_settings.clone()), network : node_settings.network })
                    }
                    ApsakdNodeKind::ExternalAsDaemon => {
                        let path = node_settings.apsakd_daemon_binary.clone();
                        Ok(ApsakdServiceEvents::StartExternalAsDaemon { path : PathBuf::from(path), config : Config::from(node_settings.clone()), network : node_settings.network })
                    }
                    ApsakdNodeKind::Remote => {
                        Ok(ApsakdServiceEvents::StartRemoteConnection { rpc_config : RpcConfig::from_node_settings(node_settings,options), network : node_settings.network })
                    }
                }

            } else {

                match &node_settings.node_kind {
                    ApsakdNodeKind::Disable => {
                        Ok(ApsakdServiceEvents::Disable { network : node_settings.network })
                    }
                    ApsakdNodeKind::Remote => {
                        Ok(ApsakdServiceEvents::StartRemoteConnection { rpc_config : RpcConfig::from_node_settings(node_settings,options), network : node_settings.network })
                    }
                }
            }
        }
    }
}
