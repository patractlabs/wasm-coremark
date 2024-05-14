fn clock_ms() -> u32 {
    use std::time::Instant;
    static STARTED: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    let elapsed = STARTED.get_or_init(Instant::now).elapsed();
    std::println!("clock_ms(): {elapsed:.2?}");
    elapsed.as_millis() as u32
}

fn wasmtime_coremark(wasm: &[u8]) -> f32 {
    let mut store = <wasmtime::Store<()>>::default();
    let engine = store.engine();
    let mut linker = wasmtime::Linker::new(engine);
    linker
        .func_wrap("env", "clock_ms", clock_ms)
        .expect("Wasmtime: failed to define `clock_ms` host function");
    let module = wasmtime::Module::new(engine, wasm)
        .expect("Wasmtime: failed to compile and validate coremark Wasm binary");
    linker
        .instantiate(&mut store, &module)
        .expect("Wasmtime: failed to instantiate coremark Wasm module")
        .get_typed_func::<(), f32>(&mut store, "run")
        .expect("Wasmtime: could not find \"run\" function export")
        .call(&mut store, ())
        .expect("Wasmtime: failed to execute \"run\" function")
}

fn wasmi_coremark(wasm: &[u8]) -> f32 {
    use wasmi::core::F32;
    let config = wasmi::Config::default();
    let engine = wasmi::Engine::new(&config);
    let mut store = wasmi::Store::new(&engine, ());
    let mut linker = <wasmi::Linker<()>>::new(&engine);
    linker
        .func_wrap("env", "clock_ms", clock_ms)
        .expect("Wasmi: failed to define `clock_ms` host function");
    let module = wasmi::Module::new(&engine, wasm)
        .expect("Wasmi: failed to compile and validate coremark Wasm binary");
    let result = linker
        .instantiate(&mut store, &module)
        .expect("Wasmi: failed to instantiate coremark Wasm module")
        .ensure_no_start(&mut store)
        .expect("Wasmi: failed to start Wasm module instance")
        .get_typed_func::<(), F32>(&mut store, "run")
        .expect("Wasmi: could not find \"run\" function export")
        .call(&mut store, ())
        .expect("Wasmi: failed to execute \"run\" function");
    result.into()
}

fn wasm3_coremark(wasm: &[u8]) -> f32 {
    use wasm3::{Environment, Module};

    let env = Environment::new().expect("Wasm3: failed to create execution environment");
    let rt = env
        .create_runtime(2048)
        .expect("Wasm3: failed to create runtime");
    let mut module = rt
        .load_module(Module::parse(&env, wasm).expect("Wasm3: failed to parse Wasm module"))
        .expect("Wasm: failed to parse coremark Wasm module");
    module
        .link_closure::<(), u32, _>("env", "clock_ms", |_ctx, _args| Ok(clock_ms()))
        .expect("Wasm3: failed to link \"clock_ms\" function");
    module
        .find_function::<(), f32>("run")
        .expect("Wasm3: failed to find exported \"run\" function in Wasm module instance")
        .call()
        .expect("Wasm3: failed to call \"run\" function")
}

fn wasmi_2_coremark(wasm: &[u8]) -> f32 {
    use wasmi_2::{
        v1::{Engine, Func, Linker, Module, Store, Extern},
        RuntimeValue,
        nan_preserving_float::F32,
    };

    let engine = Engine::default();
    let mut store = Store::new(&engine, ());

    let mut linker = <Linker<()>>::new();
    let clock_ms = Func::wrap(&mut store, || clock_ms() as i32);
    linker.define("env", "clock_ms", clock_ms)
        .expect("failed to define `clock_ms` for wasmi v1");

    let module = Module::new(&engine, wasm)
        .expect("compiling and validating Wasm module failed in wasmi v1 coremark");
    let instance = linker
        .instantiate(&mut store, &module)
        .expect("linking module core-mark failed in wasmi v1")
        .ensure_no_start(&mut store)
        .expect("failed to start module instance in wasmi v1");
    let mut result = RuntimeValue::F32(F32::from(0.0));
    let run = instance
        .get_export(&store, "run")
        .and_then(Extern::into_func)
        .expect("could not find function `run` in the coremark `.wasm`");
    run.call(&mut store, &[], core::slice::from_mut(&mut result))
        .expect("failed running coremark in wasmi v1");
    match result {
        RuntimeValue::F32(value) => value.into(),
        unexpected => panic!("wasmi v1 result expected `F32` but found: {:?}", unexpected),
    }
}

fn main() {
    let args = std::env::args().collect::<Vec<String>>();
    let help = || println!("usage: {} [wasmtime|wasm3|wasmi: string]", args[0]);
    let coremark_wasm = include_bytes!("coremark-minimal.wasm");

    match args.len() {
        2 => {
            let engine = args[1].as_str();

            println!(
                "Running Coremark 1.0 using {}... [should take 12..20 seconds]",
                engine
            );

            match engine {
                "wasmtime" => println!("Result: {}", wasmtime_coremark(coremark_wasm)),
                "wasm3" => println!("Result: {}", wasm3_coremark(coremark_wasm)),
                "wasmi" => println!("Result: {}", wasmi_coremark(coremark_wasm)),
                "wasmi2" => println!("Result: {}", wasmi_2_coremark(coremark_wasm)),
                _ => help(),
            }
        }
        _ => help(),
    }
}
