use crt_marshal_host::CtxHasMainMemory;

pub struct RuntimeContext {
    pub wasi: wasmtime_wasi::WasiCtx,
    pub memory: Option<wasmtime::Memory>,
}

impl CtxHasMainMemory for RuntimeContext {
    fn extract_main_memory<'a>(
        caller: &'a mut wasmtime::Caller<'_, Self>,
    ) -> (&'a mut [u8], &'a mut Self) {
        let memory = caller.data().memory.unwrap();
        memory.data_and_store_mut(&mut *caller)
    }
}
