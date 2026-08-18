use cyanrip_rs::cli::{CliAction, SUPPORTED_OUTPUTS_HELP, parse_from_env};

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
            println!("verify-log mode requested for: {path}");
            return;
        }
        CliAction::Run => {}
    }

    let settings = cfg.settings;
    println!(
        "cyanrip-rs CLI mapped: outputs={}, paranoia={}, retries={}",
        settings.outputs.len(),
        settings.paranoia_level,
        settings.max_retries
    );
}
