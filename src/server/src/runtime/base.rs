use crt_marshal_host::WasmPtr;

pub struct RuntimeContext {
    pub wasi: wasmtime_wasi::WasiCtx,
    pub memory: Option<wasmtime::Memory>,
    pub guest_alloc: Option<crt_marshal_host::MarshaledTypedFunc<(u32, u32), WasmPtr<()>>>,
}

impl crt_marshal_host::StoreHasMemory for RuntimeContext {
    fn main_memory(&self) -> wasmtime::Memory {
        self.memory.unwrap()
    }

    fn alloc_func(&self) -> crt_marshal_host::MarshaledTypedFunc<(u32, u32), WasmPtr<()>> {
        self.guest_alloc.unwrap()
    }
}