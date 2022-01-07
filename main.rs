/// ENV clock_ms
fn clock_ms() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Clock may have gone backwards")
        .as_millis() as u32
}

/// Wasmtime coremark
fn wasmtime_coremark(b: &[u8]) -> f32 {
    use wasmtime::{Linker, Module, Store, Val};

    let store = Store::default();
    let mut linker = Linker::new(&store);

    linker
        .func("env", "clock_ms", || clock_ms())
        .expect("Link clock_ms failed");

    if let Val::F32(res) = linker
        .instantiate(
            &Module::new(store.engine(), b).expect("Init wasm module failed in wasmtime coremark"),
        )
        .expect("Link module core-mark failed")
        .get_func("run")
        .expect("Could not find function `run` in the coremark")
        .call(&[])
        .expect("Failed running coremark in wasmtime")[0]
    {
        f32::from_bits(res)
    } else {
        panic!("Failed running coremark in wasmtime");
    }
}

/// Wasm3 coremark
fn wasm3_coremark(b: &[u8]) -> f32 {
    use wasm3::{Environment, Module};

    let env = Environment::new().expect("Unable to create environment");
    let rt = env.create_runtime(2048).expect("Unable to create runtime");
    let mut module = rt
        .load_module(Module::parse(&env, &b[..]).expect("Unable to parse module"))
        .expect("Unable to load module");

    module
        .link_function::<(), u32>("env", "clock_ms", clock_ms_wrap)
        .expect("Unable to link function");

    module
        .find_function::<(), f32>("run")
        .expect("Unable to find function")
        .call()
        .expect("Calling coremark failed in wasm3")
}

wasm3::make_func_wrapper!(clock_ms_wrap: clock_ms() -> u32);

/// wasmi coremark
fn wasmi_coremark(b: &[u8]) -> f32 {
    use wasmi::{
        Error, Externals, FuncInstance, FuncRef, HostError, ImportsBuilder, ModuleImportResolver,
        ModuleInstance, RuntimeArgs, RuntimeValue, Signature, Trap, TrapKind, ValueType,
    };

    #[derive(Debug)]
    struct E;

    impl core::fmt::Display for E {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            write!(f, "Execute imports functions in env failed")
        }
    }

    impl HostError for E {}

    /// Env resolver for wasmi
    struct EnvResolver;

    impl Externals for EnvResolver {
        fn invoke_index(
            &mut self,
            index: usize,
            _args: RuntimeArgs,
        ) -> Result<Option<RuntimeValue>, Trap> {
            match index {
                0 => Ok(Some(RuntimeValue::from(clock_ms()))),
                _ => Err(Trap::new(TrapKind::Host(Box::new(E)))),
            }
        }
    }

    impl ModuleImportResolver for EnvResolver {
        fn resolve_func(&self, field_name: &str, _signature: &Signature) -> Result<FuncRef, Error> {
            Ok(FuncInstance::alloc_host(
                Signature::new(&[][..], Some(ValueType::I32)),
                match field_name {
                    "clock_ms" => 0,
                    _ => {
                        return Err(Error::Instantiation(format!(
                            "Export {} not found",
                            field_name
                        )))
                    }
                },
            ))
        }
    }

    if let RuntimeValue::F32(res) = ModuleInstance::new(
        &wasmi::Module::from_buffer(b).expect("Failed to parse parsed `coremark-mininal.wasm`"),
        &ImportsBuilder::default().with_resolver("env", &EnvResolver),
    )
    .expect("Init wasmi module of coremark-minial failed")
    .assert_no_start()
    .invoke_export("run", &[], &mut EnvResolver)
    .expect("Failed running coremark in wasmi")
    .expect("Failed running coremark in wasmi")
    {
        f32::from(res)
    } else {
        panic!("Failed running coremark in wasmi");
    }
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
