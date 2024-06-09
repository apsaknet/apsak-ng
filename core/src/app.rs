use crate::events::ApplicationEventsChannel;
use crate::interop::Adaptor;
use crate::result::Result;
use crate::servers::parse_default_servers;
use cfg_if::cfg_if;
use apsak_ng_core::runtime;
use apsak_ng_core::settings::Settings;
use apsak_wallet_core::api::WalletApi;
use std::sync::Arc;
use workflow_i18n::*;
use workflow_log::*;

// pub const APSAK_NG_ICON_SVG: &[u8] = include_bytes!("../../resources/images/apsak.svg");
pub const APSAK_NG_ICON_SVG: &[u8] = include_bytes!("../resources/images/apsak-node-dark.svg");
pub const APSAK_NG_ICON_TRANSPARENT_SVG: &[u8] =
    include_bytes!("../resources/images/apsak-node-transparent.svg");
pub const APSAK_NG_LOGO_SVG: &[u8] = include_bytes!("../resources/images/apsak-ng.svg");
pub const I18N_EMBEDDED: &str = include_str!("../resources/i18n/i18n.json");
pub const BUILD_TIMESTAMP: &str = env!("VERGEN_BUILD_TIMESTAMP");
pub const GIT_DESCRIBE: &str = env!("VERGEN_GIT_DESCRIBE");
pub const GIT_SHA: &str = env!("VERGEN_GIT_SHA");
pub const RUSTC_CHANNEL: &str = env!("VERGEN_RUSTC_CHANNEL");
pub const RUSTC_COMMIT_DATE: &str = env!("VERGEN_RUSTC_COMMIT_DATE");
pub const RUSTC_COMMIT_HASH: &str = env!("VERGEN_RUSTC_COMMIT_HASH");
pub const RUSTC_HOST_TRIPLE: &str = env!("VERGEN_RUSTC_HOST_TRIPLE");
pub const RUSTC_LLVM_VERSION: &str = env!("VERGEN_RUSTC_LLVM_VERSION");
pub const RUSTC_SEMVER: &str = env!("VERGEN_RUSTC_SEMVER");
pub const CARGO_TARGET_TRIPLE: &str = env!("VERGEN_CARGO_TARGET_TRIPLE");
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const CODENAME: &str = "DNA";

#[derive(Default, Clone)]
pub struct ApplicationContext {
    pub wallet_api: Option<Arc<dyn WalletApi>>,
    pub application_events: Option<ApplicationEventsChannel>,
    pub adaptor: Option<Arc<Adaptor>>,
}

impl ApplicationContext {
    pub fn new(
        wallet_api: Option<Arc<dyn WalletApi>>,
        application_events: Option<ApplicationEventsChannel>,
        adaptor: Option<Arc<Adaptor>>,
    ) -> Self {
        Self {
            wallet_api,
            application_events,
            adaptor,
        }
    }
}

cfg_if! {
    if #[cfg(not(target_arch = "wasm32"))] {
        use apsakd_lib::daemon::{
            create_core,
            // DESIRED_DAEMON_SOFT_FD_LIMIT,
            // MINIMUM_DAEMON_SOFT_FD_LIMIT
        };
        use apsakd_lib::args::Args as NodeArgs;
        use apsak_utils::fd_budget;
        use apsak_core::signals::Signals;
        use clap::ArgAction;
        use crate::utils::*;
        use runtime::panic::*;
        use std::fs;

        const DESIRED_DAEMON_SOFT_FD_LIMIT: u64 = 4 * 1024;
        const MINIMUM_DAEMON_SOFT_FD_LIMIT: u64 = 2 * 1024;

        #[derive(Debug)]
        enum I18n {
            Import,
            Export,
            Reset,
        }

        enum Args {
            I18n { op : I18n },
            Cli,
            Kng {
                reset_settings : bool,
                disable : bool,
            },
            Apsakd { args : Box<NodeArgs> },
        }

        fn parse_args() -> Args {
            #[allow(unused)]
            use clap::{arg, command, Arg, Command};
            use std::env::{args,var};
            use std::iter::once;

            if args().any(|arg| arg == "--daemon") || var("APSAK_NG_DAEMON").is_ok() {
                let args = once("apsakd".to_string()).chain(args().skip(1).filter(|arg| arg != "--daemon"));//.collect::<Vec<String>>();
                match NodeArgs::parse(args) {
                    Ok(args) => Args::Apsakd { args : Box::new(args) },
                    Err(err) => {
                        println!("{err}");
                        std::process::exit(1);
                    }
                }
            } else {

                let cmd = Command::new("apsak-ng")

                    .about(format!("apsak-ng v{VERSION}-{GIT_DESCRIBE} (rusty-apsak v{})", apsak_wallet_core::version()))
                    .arg(arg!(--version "Display software version"))
                    .arg(arg!(--disable "Disable node services when starting"))
                    .arg(arg!(--daemon "Run as Rusty apsaK p2p daemon"))
                    .arg(arg!(--cli "Run as Rusty apsaK Cli Wallet"))
                    .arg(
                        Arg::new("reset-settings")
                        .long("reset-settings")
                        .action(ArgAction::SetTrue)
                        .help("Reset apsak-ng settings")
                    )
                    .subcommand(
                        Command::new("i18n").hide(true)
                        .about("apsak-ng i18n user interface translation")
                        .subcommand(
                            Command::new("import")
                                .about("import JSON files suffixed with language codes (*_en.json, *_de.json, etc.)")
                        )
                        .subcommand(
                            Command::new("export")
                                .about("export default 'en' translations as JSON")
                        )
                        .subcommand(
                            Command::new("reset")
                                .about("reset i18n data file")
                        )
                    )
                    ;

                    let matches = cmd.get_matches();

                    if matches.get_one::<bool>("version").cloned().unwrap_or(false) {
                        println!("v{VERSION}-{GIT_DESCRIBE}");
                        std::process::exit(0);
                    } else if matches.get_one::<bool>("cli").cloned().unwrap_or(false) {
                        Args::Cli
                    } else if let Some(matches) = matches.subcommand_matches("i18n") {
                        if let Some(_matches) = matches.subcommand_matches("import") {
                            Args::I18n { op : I18n::Import }
                        } else if let Some(_matches) = matches.subcommand_matches("export") {
                            Args::I18n { op : I18n::Export }
                        } else if let Some(_matches) = matches.subcommand_matches("reset") {
                            Args::I18n { op : I18n::Reset }
                        } else {
                            println!();
                            println!("please specify a valid i18n subcommand");
                            std::process::exit(1);
                        }
                    } else {
                        let disable = matches.get_one::<bool>("disable").cloned().unwrap_or(false);
                        let reset_settings = matches.get_one::<bool>("reset-settings").cloned().unwrap_or(false);

                        Args::Kng { reset_settings, disable }
                    }
            }
        }

        // pub async fn apsak_ng_main(wallet_api : Option<Arc<dyn WalletApi>>, application_events : Option<ApplicationEventsChannel>, _adaptor: Option<Arc<Adaptor>>) -> Result<()> {
        pub async fn apsak_ng_main(application_context : ApplicationContext) -> Result<()> {
            use std::sync::Mutex;

            let ApplicationContext { wallet_api, application_events, adaptor: _ } = application_context;

            match try_set_fd_limit(DESIRED_DAEMON_SOFT_FD_LIMIT) {
                Ok(limit) => {
                    if limit < MINIMUM_DAEMON_SOFT_FD_LIMIT {
                        println!();
                        println!("| Current OS file descriptor limit (soft FD limit) is set to {limit}");
                        println!("| The apsakd node requires a setting of at least {DESIRED_DAEMON_SOFT_FD_LIMIT} to operate properly.");
                        println!("| Please increase the limits using the following command:");
                        println!("| ulimit -n {DESIRED_DAEMON_SOFT_FD_LIMIT}");
                        println!();
                    }
                }
                Err(err) => {
                    println!();
                    println!("| Unable to initialize the necessary OS file descriptor limit (soft FD limit) to: {}", err);
                    println!("| The apsakd node requires a setting of at least {DESIRED_DAEMON_SOFT_FD_LIMIT} to operate properly.");
                    println!();
                }
            }


            match parse_args() {
                Args::Cli => {
                    use apsak_cli_lib::*;
                    // cli instantiates a custom panic handler
                    let result = apsak_cli(TerminalOptions::new().with_prompt("$ "), None).await;
                    if let Err(err) = result {
                        println!("{err}");
                    }
                }
                Args::Apsakd{ args } => {
                    init_ungraceful_panic_handler();
                    let fd_total_budget = fd_budget::limit() - args.rpc_max_clients as i32 - args.inbound_limit as i32 - args.outbound_target as i32;
                    let (core, _) = create_core(*args, fd_total_budget);
                    Arc::new(Signals::new(&core)).init();
                    core.run();
                }

                Args::I18n {
                    op
                } => {
                    init_ungraceful_panic_handler();
                    manage_i18n(op)?;
                }

                Args::Kng { reset_settings, disable } => {
                    init_graceful_panic_handler();

                    workflow_log::set_colors_enabled(true);

                    println!("apsak-ng v{VERSION}-{GIT_DESCRIBE} (rusty-apsak v{})", apsak_wallet_core::version());

                    // Log to stderr (if you run with `RUST_LOG=debug`).
                    env_logger::init();

                    set_log_level(LevelFilter::Info);

                    parse_default_servers();

                    let mut settings = if reset_settings {
                        println!("Resetting apsak-ng settings on user request...");
                        Settings::default().store_sync()?.clone()
                    } else {
                        Settings::load().await.unwrap_or_else(|err| {
                            log_error!("Unable to load settings: {err}");
                            Settings::default()
                        })
                    };

                    // println!("settings: {:#?}", settings);

                    let i18n_json_file = i18n_storage_file()?;
                    let i18n_json_file_load = i18n_json_file.clone();
                    let i18n_json_file_store = i18n_json_file.clone();
                    i18n::Builder::new(settings.language_code.as_str(), "en")
                        .with_static_json_data(I18N_EMBEDDED)
                        .with_string_json_data(i18n_json_file.exists().then(move ||{
                            fs::read_to_string(i18n_json_file_load)
                        }).transpose()?)
                        .with_store(move |json_data: &str| {
                            Ok(fs::write(&i18n_json_file_store, json_data)?)
                        })
                        .try_init()?;

                    if disable {
                        settings.node.node_kind = apsak_ng_core::settings::ApsakdNodeKind::Disable;
                    }

                    let runtime: Arc<Mutex<Option<runtime::Runtime>>> = Arc::new(Mutex::new(None));
                    let delegate = runtime.clone();

                    let window_frame = true;

                    let mut viewport = egui::ViewportBuilder::default()
                        .with_resizable(true)
                        .with_title(i18n("apsaK NG"))
                        .with_min_inner_size([400.0,320.0])
                        .with_inner_size([1000.0,600.0])
                        .with_icon(svg_to_icon_data(APSAK_NG_ICON_SVG, FitTo::Size(256,256)));

                    if window_frame {
                        viewport = viewport
                            .with_decorations(false)
                            .with_transparent(true);
                    }

                    let native_options = eframe::NativeOptions {
                        persist_window : true,
                        viewport,
                        ..Default::default()
                    };

                    // let application_events = ApplicationEventsChannel::unbounded();

                    eframe::run_native(
                        "apsaK NG",
                        native_options,
                        Box::new(move |cc| {
                            let runtime = runtime::Runtime::new(&cc.egui_ctx, &settings, wallet_api, application_events, None);
                            delegate.lock().unwrap().replace(runtime.clone());
                            runtime::signals::Signals::bind(&runtime);
                            runtime.start();

                            Box::new(apsak_ng_core::Core::new(cc, runtime, settings, window_frame))
                        }),
                    )?;

                    let runtime = runtime.lock().unwrap().take().unwrap();
                    runtime.shutdown().await;

                }
            }

            Ok(())
        }
    } else {

        // use crate::result::Result;
        // use crate::adaptor::Adaptor;

        // pub async fn apsak_ng_main(wallet_api : Option<Arc<dyn WalletApi>>, application_events : Option<ApplicationEventsChannel>, adaptor: Option<Arc<Adaptor>>) -> Result<()> {
        pub async fn apsak_ng_main(application_context : ApplicationContext) -> Result<()> {
            use workflow_dom::utils::document;

            let ApplicationContext { wallet_api, application_events, adaptor } = application_context;

            // ------------------------------------------------------------
            // ------------------------------------------------------------
            // ------------------------------------------------------------
            // log_info!("Sending ping request...");
            // let wallet_api = wallet_api.expect("wallet_api is None");
            // let v = wallet_api.ping(1).await.expect("ping failed");
            // log_info!("Ping response received '{v}' (should be 2) ...");

            // ------------------------------------------------------------
            // ------------------------------------------------------------
            // ------------------------------------------------------------


            // Redirect `log` message to `console.log` and friends:
            eframe::WebLogger::init(log::LevelFilter::Debug).ok();
            let web_options = eframe::WebOptions::default();

            apsak_core::log::set_log_level(apsak_core::log::LevelFilter::Info);

            parse_default_servers();

            let settings = Settings::load().await.unwrap_or_else(|err| {
                log_error!("Unable to load settings: {err}");
                Settings::default()
            });

            i18n::Builder::new(settings.language_code.as_str(), "en")
                .with_static_json_data(I18N_EMBEDDED)
                .try_init()?;

            // wasm_bindgen_futures::spawn_local(async {
            use workflow_log::*;
            log_info!("Welcome to apsaK NG! Have a great day!");

            if let Some(element) = document().get_element_by_id("loading") {
                element.remove();
            }

            eframe::WebRunner::new()
                .start(
                    "apsak-ng",
                    web_options,
                    Box::new(move |cc| {

                        // wallet_api.ping()

                        // let adaptor = apsak_ng_core::adaptor::Adaptor::new(runtime.clone());
                        // let window = web_sys::window().expect("no global `window` exists");
                        // js_sys::Reflect::set(
                        //     &window,
                        //     &JsValue::from_str("adaptor"),
                        //     &JsValue::from(adaptor),
                        // ).expect("failed to set adaptor");

                        let runtime = runtime::Runtime::new(&cc.egui_ctx, &settings, wallet_api, application_events, adaptor);
                        runtime.start();



                        Box::new(apsak_ng_core::Core::new(cc, runtime, settings, false))
                    }),
                )
                .await
                .expect("failed to start eframe");

                // log_info!("shutting down...");
            // });

            Ok(())
        }
    }
}

cfg_if! {
    if #[cfg(not(target_arch = "wasm32"))] {
        fn manage_i18n(op : I18n) -> Result<()> {
            if matches!(op, I18n::Reset) {
                println!("resetting i18n data file");
                i18n::create(i18n_storage_file()?)?;
                return Ok(());
            }

            let i18n_json_file = i18n_storage_file()?;
            let i18n_json_file_store = i18n_storage_file()?;
            i18n::Builder::new("en", "en")
                .with_static_json_data(I18N_EMBEDDED)
                .with_string_json_data(i18n_json_file.exists().then(move ||{
                    fs::read_to_string(i18n_json_file)
                }).transpose()?)
                .with_store(move |json_data: &str| {
                    Ok(fs::write(&i18n_json_file_store, json_data)?)
                })
                .try_init()?;

            match op {
                I18n::Import => {
                    let source_folder = i18n_storage_folder()?;
                    println!("importing translation files from: '{}'", source_folder.display());
                    i18n::import_translation_files(source_folder,false)?;
                }
                I18n::Export => {
                    let mut target_folder = if let Some(cwd) = try_cwd_repo_root()? {
                        cwd.join("resources").join("i18n")
                    } else {
                        std::env::current_dir()?
                    };
                    target_folder.push("apsak-ng_en.json");
                    println!("exporting default language to: '{}'", target_folder.display());
                    i18n::export_default_language(move |json_data: &str| {
                        Ok(fs::write(&target_folder, json_data)?)
                    })?;
                }
                _ => unreachable!()
            }

            Ok(())
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn try_set_fd_limit(limit: u64) -> Result<u64> {
    cfg_if::cfg_if! {
        if #[cfg(target_os = "windows")] {
            Ok(rlimit::setmaxstdio(limit as u32).map(|v| v as u64)?)
        } else if #[cfg(unix)] {
            Ok(rlimit::increase_nofile_limit(limit)?)
        }
    }
}
