use std::{error::Error, io, process, time::Duration};

use clap::{command, Parser, ValueHint};
use log::{debug, error, info, LevelFilter};
use rand::Rng;

use pleezer::{arl::Arl, config::Config, gateway::Gateway, player::Player, remote};

/// Profile to display when not built in release mode.
#[cfg(debug_assertions)]
const BUILD_PROFILE: &str = "debug";
/// Profile to display when not built release mode.
#[cfg(not(debug_assertions))]
const BUILD_PROFILE: &str = "release";

/// Group name for mutually exclusive logging options.
const ARGS_GROUP_LOGGING: &str = "logging";

/// Command line arguments as parsed by `clap`.
#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord, Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Configuration file with the `arl` cookie.
    ///
    /// For security, make this file accessible only to the owner of the Deezer
    /// account. Do not share this file or its contents with anyone.
    #[arg(short, long, value_name = "FILE", value_hint = ValueHint::FilePath, default_value_t = String::from("arl.toml"))]
    arl_file: String,

    /// Device name
    ///
    /// [default: hostname]
    #[arg(short, long, value_hint = ValueHint::Hostname)]
    name: Option<String>,

    /// Allow session interruption
    ///
    /// When enabled, active connections will be disconnected and replaced by
    /// later connections. When disabled, this device will still show up but
    /// not accept new connections when already connected.
    #[arg(short, long, default_value_t = true)]
    interruptions: bool,

    /// Quiet; no logging
    #[arg(short, long, default_value_t = false, group = ARGS_GROUP_LOGGING)]
    quiet: bool,

    /// Verbose logging
    ///
    /// Specify twice to be extra verbose.
    #[arg(short, long, action = clap::ArgAction::Count, group = ARGS_GROUP_LOGGING)]
    verbose: u8,
}

/// Initializes the logger facade. The logging level is determined as follows,
/// in order of precedence from highest to lowest:
/// 1. Command line arguments
/// 2. `RUST_LOG` environment variable
/// 3. Hard coded default
///
/// # Parameters
///
/// - `config`: an `Args` struct with `quiet` field as `bool` and `verbose` as
///   `usize`.
///
/// # Panics
///
/// Panics when a logger facade is already initialized.
fn init_logger(config: &Args) {
    let mut logger = env_logger::Builder::from_env(
        // Note: if you change the default logging level here, then you should
        // probably also change the verbosity levels below.
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    );

    if config.quiet || config.verbose > 0 {
        let level = match config.verbose {
            0 => {
                // Quiet and verbose are mutually exclusive, and `verbose` is 0
                // by default. So this arm means: quiet mode.
                LevelFilter::Off
            }
            1 => LevelFilter::Debug,
            _ => LevelFilter::Trace,
        };

        // Filter log messages of external crates.
        logger.filter_module(module_path!(), level);
    }

    logger.init();
}

/// Loads an `arl` from a `arl_file`.
///
/// # Parameters
///
/// - `arl_file`: a path to a TOML file that contains an `arl`.
///
/// # Returns
///
/// - `Ok`: a `String` with the `arl` to access the Deezer streaming service.
///
/// # Errors
///
/// Will return `Err` if:
/// - loading `arl_file` fails
fn load_arl(arl_file: &str) -> io::Result<Arl> {
    let arl = Arl::from_file(arl_file);

    if let Err(ref e) = arl {
        if e.kind() == io::ErrorKind::NotFound {
            info!("read the documentation on how to set your ARL in {arl_file}");
        }
    }

    arl
}

async fn run(args: Args) -> Result<(), Box<dyn Error>> {
    let arl = load_arl(&args.arl_file)?;

    let mut config = Config::default();
    config.interruptions = args.interruptions;
    config.device_name = args
        .name
        .or_else(|| sysinfo::System::host_name().clone())
        .unwrap_or_else(|| config.app_name.clone());

    let session = Gateway::new(&config, &arl)?;
    let player = Player::new();
    let mut client = remote::Client::new(&config, session, player, true)?;

    // Restart after sleeping some duration to prevent accidental denial of
    // service attacks on the Deezer infrastructure. The initial connection
    // happens immediately.
    let restart_timer = tokio::time::sleep(Duration::ZERO);
    tokio::pin!(restart_timer);

    // Main application loop. This restarts the new remote client when it gets
    // disconnected for whatever reason. This could be from a network failure
    // on either end or simply a disconnection from the user. In this case, the
    // session is refreshed with possibly new user data.
    loop {
        tokio::select! {
            // Prioritize shutdown signals.
            biased;

            _ = tokio::signal::ctrl_c() => {
                info!("shutting down gracefully");
                client.stop().await;
                break Ok(())
            }

            result = client.start(), if restart_timer.is_elapsed() => {
                if let Err(e) = result {
                    error!("{e}");
                }

                // Sleep with jitter to prevent thundering herds.
                let duration = Duration::from_millis(rand::thread_rng().gen_range(5_000..6_000));
                info!("restarting in {:.1}s", duration.as_secs_f32());
                restart_timer.as_mut().reset(tokio::time::Instant::now() + duration);
            }

            () = &mut restart_timer, if !restart_timer.is_elapsed() => {}
        }
    }
}

#[tokio::main]
async fn main() {
    // `clap` handles our command line arguments and help text.
    let args = Args::parse();
    init_logger(&args);

    // Dump command line arguments before we do anything more.
    // This aids in debugging of whatever comes next.
    debug!("Command {:#?}", args);

    let cmd = command!();
    let name = cmd.get_name().to_string();
    let version = cmd.get_version().unwrap_or("UNKNOWN").to_string();
    let lang = String::from("en");

    info!("starting {name}/{version}; {BUILD_PROFILE}; {lang}");

    if let Err(e) = run(args).await {
        error!("{e}");
        process::exit(1);
    }
}
