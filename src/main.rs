use std::{error::Error, io, os::unix::ffi::OsStrExt, process, time::Duration};

use clap::{command, Parser, ValueHint};
use log::{debug, error, info, LevelFilter};
use rand::Rng;

use pleezer::{arl, config::Config, connect::Connect, session::Session};

/// Profile to display when not built in release mode.
#[cfg(debug_assertions)]
const BUILD_PROFILE: &str = "debug";
/// Profile to display when not built release mode.
#[cfg(not(debug_assertions))]
const BUILD_PROFILE: &str = "release";

/// Group name for mutually exclusive logging options.
const ARGS_GROUP_LOGGING: &str = "logging";

/// Command line arguments as parsed by `clap`.
#[derive(Parser, Debug, Default)]
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

    /// Quiet; no logging
    #[arg(long, default_value_t = false, group = ARGS_GROUP_LOGGING)]
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
fn load_arl(arl_file: &str) -> io::Result<String> {
    let arl = arl::load(arl_file);

    if let Err(ref e) = arl {
        if e.kind() == io::ErrorKind::NotFound {
            info!("read the documentation on how to set your ARL in {arl_file}");
        }
    }

    arl
}

// TODO: fn docs
async fn run(args: &Args) -> Result<(), Box<dyn Error>> {
    let arl = load_arl(&args.arl_file)?;

    let mut config = Config::default();
    let player_name = args
        .name
        .as_ref()
        .map_or_else(|| try_hostname(), |name| Ok(name.clone()))?;
    config.device_name = player_name;

    let session = Session::new(&config, &arl)?;
    Connect::new(&config, session, true).await?;
    
    loop here not there
    also do not immediatey start on new()

    Ok(())
}

/// Gets the system hostname.
///
/// # Returns
///
/// - `Ok`: a `String` with the system hostname in UTF-8.
///
/// # Errors
///
/// Will return `Err` if:
/// - the hostname cannot be gotten
/// - the hostname cannot be parsed as UTF-8
fn try_hostname() -> io::Result<String> {
    match hostname::get() {
        Ok(hostname) => Ok(String::from_utf8(hostname.as_bytes().to_vec())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?),
        Err(e) => Err(e),
    }
}

/// TODO
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

    loop {
        if let Err(e) = run(&args).await {
            error!("{e}");

            // Sleep with jitter to prevent thundering herds.
            let sleep_with_jitter = rand::thread_rng().gen_range(1..=5);
            info!("retrying in {sleep_with_jitter} seconds...");
            tokio::time::sleep(Duration::from_secs(sleep_with_jitter)).await;
        }
    }
}
