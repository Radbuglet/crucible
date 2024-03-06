use std::{
    mem,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
};

use anyhow::Context;
use crt_marshal_host::{
    bind_to_linker, ContextMemoryExt, MemoryRead, WasmFuncOnHost, WasmPtr, WasmStr,
};
use generational_arena::{Arena, Index};
use tokio::sync::Mutex;

use super::logger::create_std_log_stream;

// === Manager === //

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct ProcessId(Index);

#[derive(Clone)]
pub struct RuntimeManager(Arc<RuntimeManagerInner>);

struct RuntimeManagerInner {
    engine: wasmtime::Engine,
    linker: wasmtime::Linker<RuntimeContext>,
    guarded: Mutex<RuntimeManagerGuarded>,
}

#[derive(Default)]
struct RuntimeManagerGuarded {
    live_processes: Arena<Arc<SharedRuntimeState>>,
    dead_processes: Vec<JoinHandle<()>>,
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
            move |mut cx: wasmtime::Caller<'_, RuntimeContext>, api_name: WasmStr| {
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
            |mut cx: wasmtime::Caller<'_, RuntimeContext>,
             data: WasmPtr<()>,
             cb: WasmFuncOnHost<(WasmPtr<()>, WasmStr)>| {
                let str = cx.alloc_str("hello world!")?;
                cb.call(cx, (data, str))?;
                Ok(())
            },
        )?;

        wasi_common::sync::add_to_linker(&mut linker, |c: &mut RuntimeContext| &mut c.wasi)?;

        Ok(Self(Arc::new(RuntimeManagerInner {
            engine,
            linker,
            guarded: Default::default(),
        })))
    }

    pub fn engine(&self) -> &wasmtime::Engine {
        &self.0.engine
    }

    pub async fn spawn(
        &self,
        prefix: &str,
        module: &wasmtime::Module,
    ) -> anyhow::Result<ProcessId> {
        // Build store
        let wasi = wasi_common::sync::WasiCtxBuilder::new()
            .stdout(Box::new(create_std_log_stream(format!("{prefix}.stdout"))))
            .stderr(Box::new(create_std_log_stream(format!("{prefix}.stderr"))))
            .build();

        let state = Arc::new(SharedRuntimeState {
            kill_switch: AtomicBool::new(false),
            thread: Mutex::new(None),
        });

        let mut store = wasmtime::Store::new(
            &self.0.engine,
            RuntimeContext {
                state: state.clone(),
                wasi,
                memory: None,
                function_table: None,
                guest_alloc: None,
            },
        );

        store.epoch_deadline_callback(|store| {
            if store.data().state.kill_switch.load(Ordering::Relaxed) {
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
        let handle = ProcessId(guarded.live_processes.insert(state));

        let thread = std::thread::spawn({
            let me = self.clone();
            let prefix = prefix.to_string();

            move || {
                if let Err(err) = me.run_process(&mut store, &instance, handle) {
                    log::error!(target: prefix.as_str(), "process crashed: {err:?}");
                } else {
                    log::info!(target: prefix.as_str(), "process terminated successfully");
                };

                // Remove from the live-processes set without adding to the dead-process list since
                // this thread doesn't really need to be joined.
                me.0.guarded.blocking_lock().live_processes.remove(handle.0);
            }
        });
        *guarded.live_processes[handle.0].thread.try_lock().unwrap() = Some(thread);

        Ok(handle)
    }

    fn run_process(
        &self,
        store: &mut wasmtime::Store<RuntimeContext>,
        instance: &wasmtime::Instance,
        handle: ProcessId,
    ) -> anyhow::Result<()> {
        instance
            .get_typed_func(&mut *store, "pre_init")?
            .call(&mut *store, ())?;

        instance
            .get_typed_func::<(), ()>(&mut *store, "_start")
            .context("failed to find server WASM main function")?
            .call(&mut *store, ())
            .context("failed to call server WASM main function")?;

        Ok(())
    }

    pub async fn shutdown(&self, pid: ProcessId) {
        let mut guarded = self.0.guarded.lock().await;
        if let Some(state) = guarded.live_processes.remove(pid.0) {
            state.kill_switch.store(true, Ordering::Relaxed);
            guarded.dead_processes.push(
                state
                    .thread
                    .try_lock()
                    .unwrap() // This mutex is only accessed while the `guarded` lock is held
                    .take()
                    .unwrap(), // The `guarded` lock is only released by `spawn` after this value has been initialized.
            );
        }
    }

    pub async fn wait_for_shutdown(&self) {
        // Increment the epoch to signal all processes to stop.
        self.0.engine.increment_epoch();

        // Join all the parked threads
        let parked = mem::take(&mut self.0.guarded.lock().await.dead_processes);
        for parked in parked {
            let _ = parked.join();
        }
    }
}

// === Process State === //

struct RuntimeContext {
    state: Arc<SharedRuntimeState>,

    // Core services
    wasi: wasi_common::WasiCtx,
    memory: Option<wasmtime::Memory>,
    function_table: Option<wasmtime::Table>,
    guest_alloc: Option<crt_marshal_host::WasmFuncOnHost<(u32, u32), WasmPtr<()>>>,
}

struct SharedRuntimeState {
    kill_switch: AtomicBool,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl crt_marshal_host::StoreHasMemory for RuntimeContext {
    fn main_memory(&self) -> wasmtime::Memory {
        self.memory.unwrap()
    }

    fn alloc_func(&self) -> crt_marshal_host::WasmFuncOnHost<(u32, u32), WasmPtr<()>> {
        self.guest_alloc.unwrap()
    }
}

impl crt_marshal_host::StoreHasTable for RuntimeContext {
    fn func_table(&self) -> wasmtime::Table {
        self.function_table.unwrap()
    }
}
