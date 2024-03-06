use std::time::Duration;

use anyhow::Context;
use clap::{Args, Parser, Subcommand};
use crucible_server::runtime::base::RuntimeManager;
use crucible_util::lang::error::{scope_err, tokio_read_file_anyhow, MultiError};
use serde::Deserialize;
use tokio::fs;

// === Clap === //

#[derive(Debug, Clone, Parser)]
#[command(about = "server runtime for crucible", long_about = None)]
struct CliArgs {
    #[command(subcommand)]
    command: CliCommand,
}

#[derive(Debug, Clone, Subcommand)]
#[command(about = "server runtime for crucible", long_about = None)]
enum CliCommand {
    Start(CliStartCommand),
}

#[derive(Debug, Clone, Args)]
struct CliStartCommand {
    #[arg(short = 'c', long = "config", name = "path to config")]
    config: Option<String>,
}

// === Config === //

#[derive(Debug, Clone, Deserialize)]
struct ConfRoot {
    bin: ConfBin,
    res: Option<ConfRes>,
}

#[derive(Debug, Clone, Deserialize)]
struct ConfBin {
    server: String,
    client: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ConfRes {
    server: Option<String>,
    shared: Option<String>,
}

// === Driver === //

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Setup debug services
    env_logger::init_from_env(
        env_logger::Env::new().default_filter_or("info,crucible_server=trace,guest=trace"),
    );

    // Parse arguments
    let cmd = CliArgs::parse();

    match &cmd.command {
        CliCommand::Start(sub) => do_cli_start_command(sub).await?,
    };

    Ok(())
}

async fn do_cli_start_command(sub: &CliStartCommand) -> anyhow::Result<()> {
    // Load config
    let conf_path = sub.config.as_deref().unwrap_or("crucible.toml");
    let conf = tokio_read_file_anyhow("config file", conf_path).await?;
    let conf_path = {
        let mut path = fs::canonicalize(conf_path)
            .await
            .context("failed to canonicalize path to config file")?;
        path.pop();
        path
    };

    let conf = String::from_utf8(conf).context("server config is invalid UTF-8")?;
    let conf = toml::from_str::<ConfRoot>(&conf)?;

    // Load binaries
    let mut mgr = RuntimeManager::new()?;
    let mut errors = MultiError::new("binary loading");

    let (server_mod, client_mod) = tokio::join!(
        tokio_read_file_anyhow("server binary", conf_path.join(conf.bin.server)),
        tokio_read_file_anyhow("client binary", conf_path.join(conf.bin.client)),
    );

    let server_mod = errors.maybe_report(scope_err(|| {
        let server_mod = server_mod?;
        let module = wasmtime::Module::new(mgr.engine(), server_mod)?;
        Ok(module)
    }));

    let client_mod = errors.maybe_report(client_mod);

    errors.finish()?;

    let server_mod = server_mod.unwrap();
    let client_mod = client_mod.unwrap();

    // Create runtime
    let pid = mgr.spawn("guest", &server_mod).await?;

    let _ = tokio::signal::ctrl_c().await;
    log::info!("Keyboard interrupt received. Giving guest 1s to shut down gracefully.");
    mgr.kill(pid).await;
    std::thread::sleep(Duration::from_millis(1000));
    mgr.force_kill_all_pending().await;

    log::info!("Goodbye!");

    Ok(())
}
