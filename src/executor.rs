use anyhow::Result;
use libcontainer::workload::Executor;
use oci_spec::runtime::Spec;
use wasmtime::Store;
use wasmtime_wasi::WasiCtx;

const EXECUTOR_NAME: &str = "wasmtime";

pub struct WasmtimeExecutor {
    pub f: wasmtime::Func,
    pub store: Store<WasiCtx>,
}

impl Executor for WasmtimeExecutor {
    fn exec(&self, _spec: &Spec) -> Result<()> {
        return Ok(());
        // let _ret = match self.f.call(&mut store, &mut [], &mut []) {
        //     Ok(_) => std::process::exit(0),
        //     Err(_) => std::process::exit(137),
        // };
    }

    fn can_handle(&self, _spec: &Spec) -> Result<bool> {
        return Ok(true);
    }

    fn name(&self) -> &'static str {
        EXECUTOR_NAME
    }
}
