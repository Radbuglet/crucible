use anyhow::Context;
use clap::{Args, Parser, Subcommand};
use crt_marshal_host::{bind_to_linker, ContextMemoryExt, MarshaledTypedFunc, MemoryRead, WasmStr};
use crucible_server::runtime::base::RuntimeContext;
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
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("INFO"));

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

    // Create engine
    let engine = wasmtime::Engine::new(wasmtime::Config::new().epoch_interruption(true))
        .context("failed to create wasm engine")?;

    // Load binaries
    let mut errors = MultiError::new("binary loading");

    let (server_mod, client_mod) = tokio::join!(
        tokio_read_file_anyhow("server binary", conf_path.join(conf.bin.server)),
        tokio_read_file_anyhow("client binary", conf_path.join(conf.bin.client)),
    );

    let server_mod = errors.maybe_report(scope_err(|| {
        let server_mod = server_mod?;
        let module = wasmtime::Module::new(&engine, server_mod)?;
        Ok(module)
    }));

    let client_mod = errors.maybe_report(client_mod);

    errors.finish()?;

    let server_mod = server_mod.unwrap();
    let client_mod = client_mod.unwrap();

    // Create linker for server module
    let mut linker = wasmtime::Linker::<RuntimeContext>::new(&engine);

    bind_to_linker(
        &mut linker,
        "crucible0",
        "get_rt_mode",
        move |_cx: wasmtime::Caller<'_, _>| Ok((1,)),
    )?;

    bind_to_linker(
        &mut linker,
        "crucible0",
        "get_api_version",
        move |mut cx: wasmtime::Caller<'_, RuntimeContext>, api_name: WasmStr| {
            let (mem, _) = cx.main_memory();
            let api_name = mem.load_str(api_name)?;
            log::info!("{api_name}");

            cx.alloc_str("0.1.0")
        },
    )?;

    wasmtime_wasi::add_to_linker(&mut linker, |c| &mut c.wasi)?;
    // linker.define_unknown_imports_as_traps(&server_mod)?;

    // Spin up runtime
    let wasi = wasmtime_wasi::WasiCtxBuilder::new()
        .inherit_stderr()
        .inherit_stdio()
        .build();

    let mut store = wasmtime::Store::new(
        &engine,
        RuntimeContext {
            wasi,
            memory: None,
            guest_alloc: None,
        },
    );
    let instance = linker
        .instantiate(&mut store, &server_mod)
        .context("failed to instantiate server WASM module")?;

    store.set_epoch_deadline(1);

    store.data_mut().memory = Some(
        instance
            .get_memory(&mut store, "memory")
            .context("failed to get main memory of server WASM module")?,
    );

    store.data_mut().guest_alloc = Some(MarshaledTypedFunc(
        instance
            .get_typed_func(&mut store, "host_alloc")
            .context("failed to get `host_alloc` export")?,
    ));

    instance
        .get_typed_func::<(), ()>(&mut store, "_start")
        .context("failed to find server WASM main function")?
        .call(store, ())
        .context("failed to call server WASM main function")?;

    Ok(())
}
