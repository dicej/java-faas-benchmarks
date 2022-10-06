use {
    anyhow::{anyhow, Result},
    std::env,
    wasmtime::{Config, Engine, Linker, Module, Store},
    wasmtime_wasi::{WasiCtx, WasiCtxBuilder},
};

fn main() -> Result<()> {
    let engine = &Engine::new(&Config::new())?;
    let module = &Module::from_file(
        engine,
        env::args()
            .nth(1)
            .ok_or_else(|| anyhow!("please specify a wasm file"))?,
    )?;
    let store = &mut Store::new(
        engine,
        WasiCtxBuilder::new()
            .arg("<wasm module>")?
            .inherit_stderr()
            .inherit_stdout()
            .build(),
    );
    let linker = &mut Linker::<WasiCtx>::new(engine);
    wasmtime_wasi::add_to_linker(linker, |context| context)?;
    linker.func_wrap("teavmMath", "log", f64::ln)?;
    linker.func_wrap("teavmMath", "sqrt", f64::sqrt)?;

    let instance_pre = linker.instantiate_pre(&mut *store, module)?;
    let instance = instance_pre.instantiate(&mut *store)?;
    let func = instance.get_typed_func::<(), (), _>(&mut *store, "_start")?;
    for argument in env::args().skip(2) {
        store.data_mut().push_arg(&argument)?;
    }

    func.call(&mut *store, ())?;

    Ok(())
}
