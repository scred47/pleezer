use std::{error::Error, io, process, time::Duration};

use clap::{command, Parser, ValueHint};
use log::{debug, error, info, LevelFilter};
use rand::Rng;

use pleezer::{arl::Arl, config::Config, player::Player, remote};

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
    /// Secrets file
    ///
    /// Ensure that the this file is kept secure and not shared publicly, as it
    /// contains sensitive information that can grant access to your Deezer
    /// account.
    #[arg(short, long, value_name = "FILE", value_hint = ValueHint::FilePath, default_value_t = String::from("secrets.toml"))]
    secrets_file: String,

    /// Player's name
    ///
    /// Set the player's name as it appears to Deezer clients.
    ///
    /// [default: system hostname]
    #[arg(short, long, value_hint = ValueHint::Hostname)]
    name: Option<String>,

    /// Prevent session interruptions
    ///
    /// Prevent other clients from taking over the connection after pleezer has
    /// connected.
    #[arg(long, default_value_t = false)]
    no_interruptions: bool,

    /// Suppresses all output except warnings and errors.
    #[arg(short, long, default_value_t = false, group = ARGS_GROUP_LOGGING)]
    quiet: bool,

    /// Enable verbose logging
    ///
    /// Specify twice for trace logging.
    #[arg(short, long, action = clap::ArgAction::Count, group = ARGS_GROUP_LOGGING)]
    verbose: u8,
}

/// Initializes the logger facade.
///
/// The logging level is determined as follows, in order of precedence from
/// highest to lowest:
/// 1. Command line arguments
/// 2. `RUST_LOG` environment variable
/// 3. Hard coded default
///
/// # Parameters
///
/// - `config`: a `&Args` with the command line arguments.
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
                LevelFilter::Warn
            }
            1 => LevelFilter::Debug,
            _ => LevelFilter::Trace,
        };

        // Filter log messages of external crates.
        logger.filter_module(module_path!(), level);
    }

    logger.init();
}

/// Loads the `arl` from a file.
///
/// # Parameters
///
/// - `arl_file`: a `&str` with the path to the file containing the `arl`.
///
/// # Returns
///
/// - `Ok`: a `String` with the `arl` to access the Deezer streaming service.
/// - `Err`: an `io::Error` if the file could not be read.
///
/// # Errors
///
/// This function returns an error if the file could not be read. This could be
/// due to the file not existing or not having the correct permissions.
fn load_arl(arl_file: &str) -> io::Result<Arl> {
    let arl = Arl::from_file(arl_file);

    if let Err(ref e) = arl {
        if e.kind() == io::ErrorKind::NotFound {
            info!("read the documentation on how to set your ARL in {arl_file}");
        }
    }

    arl
}

/// Main application loop.
///
/// # Parameters
///
/// - `args`: a `Args` with the command line arguments.
///
/// # Returns
///
/// - `Ok`: a `()` when the application exits successfully.
/// - `Err`: a `Box<dyn Error>` when an error occurs.
///
/// # Errors
///
/// This function returns an error when an error occurs. This could be due to
/// the user interrupting the application or an unrecoverable network error.
async fn run(args: Args) -> Result<(), Box<dyn Error>> {
    let arl = load_arl(&args.secrets_file)?;

    let mut config = Config::with_arl(arl);
    config.interruptions = !args.no_interruptions;
    config.device_name = args
        .name
        .or_else(|| sysinfo::System::host_name().clone())
        .unwrap_or_else(|| config.app_name.clone());

    let player = Player::new();
    let mut client = remote::Client::new(&config, player, true)?;

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

                // Sleep with jitter to prevent thundering herds. Subsecond
                // precision further prevents that by spreading requests
                // when users are launching this from some crontab.
                let duration = Duration::from_millis(rand::thread_rng().gen_range(5_000..6_000));
                info!("restarting in {:.1}s", duration.as_secs_f32());
                restart_timer.as_mut().reset(tokio::time::Instant::now() + duration);
            }

            () = &mut restart_timer, if !restart_timer.is_elapsed() => {}
        }
    }
}

/// Main entry point of the application.
///
/// This function initializes the logger facade, parses the command line
/// arguments, and starts the main application loop.
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
