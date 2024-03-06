use std::{
    collections::HashMap,
    mem,
    sync::{
        atomic::{self, AtomicBool},
        mpsc::{channel as std_channel, Receiver as StdReceiver, Sender as StdSender},
        Arc,
    },
    time::Instant,
};

use anyhow::Context;
use crt_marshal_host::{
    bind_to_linker, ContextMemoryExt, MemoryRead, WasmFuncOnHost, WasmPtr, WasmStr,
};
use generational_arena::{Arena, Index};
use tokio::{sync::Mutex, task::JoinHandle};

use super::logger::create_std_log_stream;

// === Manager === //

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct ProcessId(Index);

#[derive(Clone)]
pub struct RuntimeManager(Arc<RuntimeManagerInner>);

struct RuntimeManagerInner {
    engine: wasmtime::Engine,
    linker: wasmtime::Linker<StoreRuntimeState>,
    guarded: Mutex<RuntimeManagerGuarded>,
}

#[derive(Default)]
struct RuntimeManagerGuarded {
    live_processes: Arena<ManagerRuntimeState>,
    condemned_processes: HashMap<ProcessId, JoinHandle<()>>,
}

impl RuntimeManager {
    pub fn new() -> anyhow::Result<Self> {
        let engine = wasmtime::Engine::new(wasmtime::Config::new().epoch_interruption(true))
            .context("failed to create wasm engine")?;

        let mut linker = wasmtime::Linker::new(&engine);

        bind_to_linker(
            &mut linker,
            "crucible0_version",
            "get_rt_mode",
            move |_cx: wasmtime::Caller<'_, _>| Ok(1u32),
        )?;

        bind_to_linker(
            &mut linker,
            "crucible0_version",
            "get_api_version",
            move |mut cx: wasmtime::Caller<'_, StoreRuntimeState>, api_name: WasmStr| {
                let (mem, _) = cx.main_memory();
                let api_name = mem.load_str(api_name)?;
                log::info!("{api_name}");

                Ok((cx.alloc_str("0.1.0")?,))
            },
        )?;

        bind_to_linker(
            &mut linker,
            "crucible0_lifecycle",
            "set_shutdown_handler",
            |mut cx: wasmtime::Caller<'_, StoreRuntimeState>,
             data: WasmPtr<()>,
             cb: WasmFuncOnHost<(WasmPtr<()>, WasmStr)>| {
                let str = cx.alloc_str("hello world!")?;
                cb.call(cx, (data, str))?;
                Ok(())
            },
        )?;

        wasi_common::sync::add_to_linker(&mut linker, |c: &mut StoreRuntimeState| &mut c.wasi)?;

        Ok(Self(Arc::new(RuntimeManagerInner {
            engine,
            linker,
            guarded: Default::default(),
        })))
    }

    pub fn engine(&self) -> &wasmtime::Engine {
        &self.0.engine
    }

    pub async fn spawn(&self, name: &str, module: &wasmtime::Module) -> anyhow::Result<ProcessId> {
        // Build store
        let wasi = wasi_common::sync::WasiCtxBuilder::new()
            .stdout(Box::new(create_std_log_stream(format!("{name}.stdout"))))
            .stderr(Box::new(create_std_log_stream(format!("{name}.stderr"))))
            .build();

        let shared = Arc::new(SharedRuntimeState {
            kill_switch: AtomicBool::new(false),
        });

        let mut store = wasmtime::Store::new(
            &self.0.engine,
            StoreRuntimeState {
                shared: shared.clone(),
                wasi,
                memory: None,
                function_table: None,
                guest_alloc: None,
            },
        );

        // Setup epoch management
        store.epoch_deadline_callback(|store| {
            // Acquire relative to the epoch to ensure that the kill-switch state is made visible
            // to this thread.
            atomic::fence(atomic::Ordering::Acquire);

            if store
                .data()
                .shared
                .kill_switch
                .load(atomic::Ordering::Relaxed)
            {
                Err(anyhow::anyhow!("kill-switch triggered"))
            } else {
                Ok(wasmtime::UpdateDeadline::Continue(1))
            }
        });

        store.set_epoch_deadline(1);

        // Spawn instance
        let instance = self
            .0
            .linker
            .instantiate(&mut store, module)
            .context("failed to instantiate server WASM module")?;

        store.data_mut().memory = Some(
            instance
                .get_memory(&mut store, "memory")
                .context("failed to get main memory of server WASM module")?,
        );

        store.data_mut().guest_alloc = Some(WasmFuncOnHost::new_not_indexed(
            instance
                .get_typed_func(&mut store, "host_alloc")
                .context("failed to get `host_alloc` export")?,
        ));

        store.data_mut().function_table = Some(
            instance
                .get_table(&mut store, "__indirect_function_table")
                .context("failed to get `__indirect_function_table` table")?,
        );

        // Create thread for process
        let mut guarded = self.0.guarded.lock().await;
        let (command_send, command_recv) = std_channel::<PuppetSignal>();

        let handle = ProcessId(guarded.live_processes.insert(ManagerRuntimeState {
            shared,
            thread: None,
            commands: command_send,
        }));

        let thread = tokio::task::spawn_blocking({
            let me = PuppetRuntimeState {
                manager: self.clone(),
                handle,
                instance,
                store,
                commands: command_recv,
                name: name.to_string(),
            };

            move || {
                me.run();
            }
        });
        guarded.live_processes[handle.0].thread = Some(thread);

        Ok(handle)
    }

    pub async fn kill(&self, pid: ProcessId) {
        let mut guarded = self.0.guarded.lock().await;
        let Some(state) = guarded.live_processes.remove(pid.0) else {
            return;
        };

        log::trace!("Killing {pid:?}");

        // Mark kill switch. We can use a relaxed ordering because a later fence will ensure that
        // these are all made visible alongside the epoch increment.
        state
            .shared
            .kill_switch
            .store(true, atomic::Ordering::Relaxed);

        // Notify the process of the soft kill so it can start cleaning up.
        let _ = state.commands.send(PuppetSignal::Killing);

        // The `guarded` lock is only released by `spawn` after this value has been initialized.
        guarded
            .condemned_processes
            .insert(pid, state.thread.unwrap());
    }

    pub async fn force_kill_all_pending(&self) {
        log::trace!("Force-killing all pending processes.");

        // Increment the epoch to signal all processes to stop.
        {
            // Ensures that the kill switch is made visible to the worker threads before the epoch
            // is incremented to avoid instances that resist the kill command.
            atomic::fence(atomic::Ordering::Release);

            // This is necessary because this operation is relaxed, meaning that the `kill_immediately`
            // flag setter could technically be reordered after it.
            self.0.engine.increment_epoch();
        }

        // Join all the parked threads
        let parked = mem::take(&mut self.0.guarded.lock().await.condemned_processes);
        for parked in parked.into_values() {
            let _ = parked.await;
        }
    }
}

// === Process State === //

struct ManagerRuntimeState {
    shared: Arc<SharedRuntimeState>,
    thread: Option<JoinHandle<()>>,
    commands: StdSender<PuppetSignal>,
}

struct PuppetRuntimeState {
    manager: RuntimeManager,
    handle: ProcessId,
    instance: wasmtime::Instance,
    store: wasmtime::Store<StoreRuntimeState>,
    commands: StdReceiver<PuppetSignal>,
    name: String,
}

impl PuppetRuntimeState {
    pub fn run(mut self) {
        log::trace!(target: self.name.as_str(), "Spawned.");

        if let Err(err) = self.run_inner() {
            log::error!(target: self.name.as_str(), "process crashed: {err:?}");
        } else {
            log::info!(target: self.name.as_str(), "process terminated successfully");
        };

        // Remove from the live-processes set and remove it from the condemned list to avoid a memory
        // leak.
        let mut guarded = self.manager.0.guarded.blocking_lock();
        guarded.live_processes.remove(self.handle.0);
        guarded.condemned_processes.remove(&self.handle);
        log::trace!(target: self.name.as_str(), "Goodbye!");
    }

    fn run_inner(&mut self) -> anyhow::Result<()> {
        // Run mandatory initialization phase
        self.instance
            .get_typed_func(&mut self.store, "pre_init")?
            .call(&mut self.store, ())?;

        self.instance
            .get_typed_func::<(), ()>(&mut self.store, "_start")
            .context("failed to find server WASM main function")?
            .call(&mut self.store, ())
            .context("failed to call server WASM main function")?;

        log::trace!(target: self.name.as_str(), "Completed startup.");

        // Start command handler
        while let Ok(sig) = self.commands.recv() {
            match sig {
                PuppetSignal::Killing => {
                    let time = Instant::now();
                    log::info!(target: self.name.as_str(), "Signal received to shutdown gracefully...");
                    // TODO: Run shutdown handler
                    log::info!(target: self.name.as_str(), "Shutdown handler ran in {:?}", time.elapsed());
                    break;
                }
                PuppetSignal::Tick => {
                    log::info!("Ticking...");
                }
            }
        }

        Ok(())
    }
}

struct StoreRuntimeState {
    shared: Arc<SharedRuntimeState>,
    wasi: wasi_common::WasiCtx,
    memory: Option<wasmtime::Memory>,
    function_table: Option<wasmtime::Table>,
    guest_alloc: Option<crt_marshal_host::WasmFuncOnHost<(u32, u32), WasmPtr<()>>>,
}

impl crt_marshal_host::StoreHasMemory for StoreRuntimeState {
    fn main_memory(&self) -> wasmtime::Memory {
        self.memory.unwrap()
    }

    fn alloc_func(&self) -> crt_marshal_host::WasmFuncOnHost<(u32, u32), WasmPtr<()>> {
        self.guest_alloc.unwrap()
    }
}

impl crt_marshal_host::StoreHasTable for StoreRuntimeState {
    fn func_table(&self) -> wasmtime::Table {
        self.function_table.unwrap()
    }
}

struct SharedRuntimeState {
    kill_switch: AtomicBool,
}

enum PuppetSignal {
    Killing,
    Tick,
}
