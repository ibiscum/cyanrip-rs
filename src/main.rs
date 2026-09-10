use cyanrip_rs::app::run_workflow;
use cyanrip_rs::cli::{CliAction, SUPPORTED_OUTPUTS_HELP, parse_from_env};
use cyanrip_rs::fun512::{LogVerify, verify_log_path};
use std::path::Path;

fn main() {
    // Diagnostic warn/error logs are on by default; RUST_LOG overrides verbosity.
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    let cfg = match parse_from_env() {
        Ok(cfg) => cfg,
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(2);
        }
    };

    match cfg.action {
        CliAction::ShowOutputsHelp => {
            println!("{SUPPORTED_OUTPUTS_HELP}");
        }
        CliAction::VerifyLog => {
            let path = cfg.settings.verify_log.as_deref().unwrap_or("<missing>");
            match verify_log_path(Path::new(path)) {
                LogVerify::Valid => {
                    println!("Log \"{path}\" checksum valid.");
                }
                LogVerify::Mismatch => {
                    println!("Log \"{path}\" checksum mismatch, the file has been modified!");
                    std::process::exit(1);
                }
                LogVerify::TrailingData => {
                    println!(
                        "Log \"{path}\" has data after the checksum, the file has been modified!"
                    );
                    std::process::exit(1);
                }
                LogVerify::NoChecksum => {
                    println!("No FUN512 checksum found in \"{path}\"!");
                    std::process::exit(1);
                }
                LogVerify::IoError => {
                    println!("Couldn't read \"{path}\"!");
                    std::process::exit(1);
                }
            }
        }
        CliAction::Run => match run_workflow(&cfg.settings) {
            Ok(Some(msg)) => {
                println!("{msg}");
            }
            Ok(None) => {}
            Err(err) => {
                eprintln!("{err}");
                std::process::exit(1);
            }
        },
    }
}
