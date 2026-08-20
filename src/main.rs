use cyanrip_rs::cli::{CliAction, SUPPORTED_OUTPUTS_HELP, parse_from_env};
use cyanrip_rs::fun512::{LogVerify, verify_log_path};
use cyanrip_rs::app::run_workflow;
use std::path::Path;

fn main() {
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
            return;
        }
        CliAction::VerifyLog => {
            let path = cfg.settings.verify_log.as_deref().unwrap_or("<missing>");
            match verify_log_path(Path::new(path)) {
                LogVerify::Valid => {
                    println!("Log \"{path}\" checksum valid.");
                    return;
                }
                LogVerify::Mismatch => {
                    println!(
                        "Log \"{path}\" checksum mismatch, the file has been modified!"
                    );
                }
                LogVerify::TrailingData => {
                    println!(
                        "Log \"{path}\" has data after the checksum, the file has been modified!"
                    );
                }
                LogVerify::NoChecksum => {
                    println!("No FUN512 checksum found in \"{path}\"!");
                }
                LogVerify::IoError => {
                    println!("Couldn't read \"{path}\"!");
                }
            }
            std::process::exit(1);
        }
        CliAction::Run => {
            match run_workflow(&cfg.settings) {
                Ok(Some(msg)) => {
                    println!("{msg}");
                    return;
                }
                Ok(None) => {
                    return;
                }
                Err(err) => {
                    eprintln!("{err}");
                    std::process::exit(1);
                }
            }
        }
    }
}
