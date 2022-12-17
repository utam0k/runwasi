use std::fs::OpenOptions;

use anyhow::{anyhow, bail, Context, Result};
use containerd_shim_wasm::sandbox::error::Error;
use containerd_shim_wasm::sandbox::oci;
use libcontainer::workload::{Executor, EMPTY};
use log::debug;
use oci_spec::runtime::Spec;
use wasmtime::{Engine, Linker, Module, Store};
use wasmtime_wasi::{sync::file::File as WasiFile, WasiCtx, WasiCtxBuilder};

use super::error::WasmtimeError;
use super::oci_wasmtime;

const EXECUTOR_NAME: &str = "wasmtime";

pub struct WasmtimeExecutor {
    pub engine: wasmtime::Engine,

    pub stdin: String,
    pub stdout: String,
    pub stderr: String,
}

impl Executor for WasmtimeExecutor {
    fn exec(&self, spec: &Spec) -> Result<()> {
        log::info!("Executing workload with wasmtime handler");
        println!("execute!!!!!!!!!!!!");

        let engine = self.engine.clone();

        let m = prepare_module(
            engine.clone(),
            &spec,
            &self.stdin,
            &self.stdout,
            &self.stderr,
        )
        .map_err(|e| Error::Others(format!("error setting up module: {}", e)))?;
        let mut store = Store::new(&self.engine, m.0);

        let mut linker = Linker::new(&self.engine);
        wasmtime_wasi::add_to_linker(&mut linker, |s| s)
            .context("cannot add wasi context to linker")?;
        let i = linker
            .instantiate(&mut store, &m.1)
            .map_err(|err| Error::Others(format!("error instantiating module: {}", err)))?;

        let f = i
            .get_func(&mut store, "_start")
            .ok_or(Error::InvalidArgument(
                "module does not have a wasi start function".to_string(),
            ))?;

        let _ret = match f.call(&mut store, &mut [], &mut []) {
            Ok(_) => std::process::exit(0),
            Err(_) => std::process::exit(137),
        };

        // let wasi = WasiCtxBuilder::new()
        //     .inherit_stdio()
        //     .args(args)
        //     .context("cannot add args to wasi context")?
        //     .envs(&envs)
        //     .context("cannot add environment variables to wasi context")?
        //     .build();
        //
        // let mut store = Store::new(&engine, wasi);
        //
        // let instance = linker
        //     .instantiate(&mut store, &module)
        //     .context("wasm module could not be instantiated")?;
        // let start = instance
        //     .get_func(&mut store, "_start")
        //     .ok_or_else(|| anyhow!("could not retrieve wasm module main function"))?;
        //
        // start
        //     .call(&mut store, &[], &mut [])
        //     .context("wasm module was not executed successfully")
    }

    fn can_handle(&self, spec: &Spec) -> Result<bool> {
        if let Some(annotations) = spec.annotations() {
            if let Some(handler) = annotations.get("run.oci.handler") {
                return Ok(handler == "wasm");
            }

            if let Some(variant) = annotations.get("module.wasm.image/variant") {
                return Ok(variant == "compat");
            }
        }

        Ok(false)
    }

    fn name(&self) -> &'static str {
        EXECUTOR_NAME
    }
}

fn prepare_module(
    engine: wasmtime::Engine,
    spec: &oci::Spec,
    stdin_path: &String,
    stdout_path: &String,
    stderr_path: &String,
) -> Result<(WasiCtx, Module), WasmtimeError> {
    debug!("opening rootfs");
    let rootfs = oci_wasmtime::get_rootfs(&spec)?;
    let args = oci::get_args(&spec);
    let env = oci_wasmtime::env_to_wasi(&spec);

    debug!("setting up wasi");
    let mut wasi_builder = WasiCtxBuilder::new()
        .args(args)?
        .envs(env.as_slice())?
        .preopened_dir(rootfs, "/")?;

    debug!("opening stdin");
    let stdin = maybe_open_stdio(stdin_path).context("could not open stdin")?;
    if let Some(sin) = stdin {
        wasi_builder = wasi_builder.stdin(Box::new(sin));
    }

    debug!("opening stdout");
    let stdout = maybe_open_stdio(stdout_path).context("could not open stdout")?;
    if let Some(sout) = stdout {
        wasi_builder = wasi_builder.stdout(Box::new(sout));
    }

    debug!("opening stderr");
    let stderr = maybe_open_stdio(stderr_path).context("could not open stderr")?;
    if let Some(serr) = stderr {
        wasi_builder = wasi_builder.stderr(Box::new(serr));
    }

    debug!("building wasi context");
    let wctx = wasi_builder.build();
    debug!("wasi context ready");

    let mut cmd = args[0].clone();
    let stripped = args[0].strip_prefix(std::path::MAIN_SEPARATOR);
    if let Some(strpd) = stripped {
        cmd = strpd.to_string();
    }

    let mod_path = oci::get_root(&spec).join(cmd);

    debug!("loading module from file");
    let module = Module::from_file(&engine, mod_path)
        .map_err(|err| Error::Others(format!("could not load module from file: {}", err)))?;

    Ok((wctx, module))
}

fn maybe_open_stdio(path: &str) -> Result<Option<WasiFile>, Error> {
    if path.is_empty() {
        return Ok(None);
    }
    match oci_wasmtime::wasi_file(path, OpenOptions::new().read(true).write(true)) {
        Ok(f) => Ok(Some(f)),
        Err(err) => match err.kind() {
            std::io::ErrorKind::NotFound => Ok(None),
            _ => Err(err.into()),
        },
    }
}
