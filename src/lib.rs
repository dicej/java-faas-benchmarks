#![deny(warnings)]
#![feature(test)]

extern crate test;

#[cfg(test)]
mod tests {
    use {
        anyhow::Result,
        dlopen::wrapper::{Container, WrapperApi},
        dlopen_derive::WrapperApi,
        gag::Gag,
        jni::{
            objects::{JObject, JValue},
            signature::{JavaType, Primitive},
            InitArgsBuilder, JNIVersion, JavaVM,
        },
        lazy_static::lazy_static,
        std::{
            ffi::{c_char, c_int, CString},
            iter,
        },
        test::Bencher,
        wasmtime::{Config, Engine, Linker, Module, Store},
        wasmtime_wasi::{WasiCtx, WasiCtxBuilder},
    };

    enum Mode {
        Direct,
        Fork,
        ForkWithPrewarm,
    }

    fn bench(bencher: &mut Bencher, test: impl Fn(), mode: Mode) {
        match mode {
            Mode::Direct => bencher.iter(test),
            Mode::Fork => bencher.iter(do_fork(test)),
            Mode::ForkWithPrewarm => {
                for _ in 0..100 {
                    test();
                }

                bencher.iter(do_fork(test))
            }
        }
    }

    fn do_fork(fun: impl Fn()) -> impl Fn() {
        move || {
            match unsafe { libc::fork() } {
                -1 => panic!("fork failed; errno: {}", errno::errno()),
                0 => {
                    // I'm the child
                    fun();

                    // Exit without running any destructors for both maximum performance and to avoid deadlocks
                    // when disposing the JVM from multiple processes:
                    unsafe { libc::_exit(0) }
                }
                child => {
                    // I'm the parent
                    let mut status = 0;
                    if -1 == unsafe { libc::waitpid(child, &mut status, 0) } {
                        panic!("waitpid failed; errno: {}", errno::errno());
                    }

                    if !(libc::WIFEXITED(status) && libc::WEXITSTATUS(status) == 0) {
                        panic!(
                            "child exited{}",
                            if libc::WIFEXITED(status) {
                                format!(" (exit status {})", libc::WEXITSTATUS(status))
                            } else if libc::WIFSIGNALED(status) {
                                format!(" (killed by signal {})", libc::WTERMSIG(status))
                            } else {
                                String::new()
                            }
                        )
                    }
                }
            }
        }
    }

    fn bench_jvm(
        bencher: &mut Bencher,
        class_name: &str,
        arguments: &[&str],
        mode: Mode,
    ) -> Result<()> {
        lazy_static! {
            static ref JVM: JavaVM = JavaVM::new(
                InitArgsBuilder::new()
                    .version(JNIVersion::V8)
                    .option(
                        "-Djava.class.path=\
                         apps/mandelbrot/target/classes:\
                         apps/nbody/target/classes:\
                         apps/pidigits/target/classes:\
                         apps/spectralnorm/target/classes:\
                         apps/simple/target/classes:\
                         apps/hello/target/classes"
                    )
                    .build()
                    .unwrap(),
            )
            .unwrap();
        }

        let env = JVM.attach_current_thread()?;

        env.call_static_method(
            "java/lang/System",
            "setOut",
            "(Ljava/io/PrintStream;)V",
            &[JValue::Object(env.new_object(
                "java/io/PrintStream",
                "(Ljava/io/OutputStream;)V",
                &[env.call_static_method(
                    "java/io/OutputStream",
                    "nullOutputStream",
                    "()Ljava/io/OutputStream;",
                    &[],
                )?],
            )?)],
        )?;

        let class = env.find_class(class_name)?;
        let method = env.get_static_method_id(class, "main", "([Ljava/lang/String;)V")?;

        let test = || {
            let _ = env
                .with_local_frame((arguments.len() + 1).try_into().unwrap(), || {
                    let args = env.new_object_array(
                        arguments.len().try_into().unwrap(),
                        "java/lang/String",
                        JObject::null(),
                    )?;

                    for (index, argument) in arguments.iter().enumerate() {
                        env.set_object_array_element(
                            args,
                            index.try_into().unwrap(),
                            env.new_string(argument)?,
                        )?;
                    }

                    env.call_static_method_unchecked(
                        class,
                        method,
                        JavaType::Primitive(Primitive::Void),
                        &[JValue::Object(args.into())],
                    )?;

                    Ok(JObject::null())
                })
                .unwrap();
        };

        bench(bencher, test, mode);

        Ok(())
    }

    fn bench_graalvm_native(
        bencher: &mut Bencher,
        class_name: &str,
        arguments: &[&str],
        mode: Mode,
    ) -> Result<()> {
        #[derive(WrapperApi)]
        struct Api {
            run_main: fn(argc: c_int, argv: *const *const c_char) -> c_int,
        }

        let container = unsafe {
            Container::<Api>::load(&format!("apps/{class_name}/target/{class_name}.so"))
        }?;

        let test = || {
            let arguments = iter::once(CString::new(class_name))
                .chain(arguments.iter().copied().map(CString::new))
                .collect::<Result<Vec<_>, _>>()
                .unwrap();

            let arguments = arguments.iter().map(|s| s.as_ptr()).collect::<Vec<_>>();

            let result =
                container.run_main(arguments.len().try_into().unwrap(), arguments.as_ptr());

            assert!(result == 0);
        };

        let _gag = Gag::stdout()?;

        bench(bencher, test, mode);

        Ok(())
    }

    fn bench_teavm(bencher: &mut Bencher, class_name: &str, arguments: &[&str]) -> Result<()> {
        let engine = &Engine::new(&Config::new())?;
        let module = &Module::from_file(
            engine,
            &format!("apps/{class_name}/target/generated/wasm/teavm-wasm/classes.wasm.opt"),
        )?;
        let linker = &mut Linker::<WasiCtx>::new(engine);
        wasmtime_wasi::add_to_linker(linker, |context| context)?;
        linker.func_wrap("teavmMath", "log", f64::ln)?;
        linker.func_wrap("teavmMath", "sqrt", f64::sqrt)?;

        let store = &mut Store::new(engine, WasiCtxBuilder::new().arg("<wasm module>")?.build());
        let instance_pre = linker.instantiate_pre(store, module)?;

        bencher.iter(|| {
            let store = &mut Store::new(
                engine,
                WasiCtxBuilder::new().arg("<wasm module>").unwrap().build(),
            );

            for argument in arguments {
                store.data_mut().push_arg(argument).unwrap();
            }

            let instance = instance_pre.instantiate(&mut *store).unwrap();

            let func = instance
                .get_typed_func::<(), (), _>(&mut *store, "_start")
                .unwrap();

            func.call(&mut *store, ()).unwrap()
        });

        Ok(())
    }

    macro_rules! benchmarks {
        ($($jvm_direct:ident $jvm_fork:ident $graal_fork:ident $teavm:ident $name:literal $($args:literal)*,)*) => ($(
            #[bench]
            fn $jvm_direct(bencher: &mut Bencher) -> Result<()> {
                bench_jvm(bencher, $name, &[$($args,)*], Mode::Direct)
            }

            #[bench]
            fn $jvm_fork(bencher: &mut Bencher) -> Result<()> {
                bench_jvm(bencher, $name, &[$($args,)*], Mode::ForkWithPrewarm)
            }

            #[bench]
            fn $graal_fork(bencher: &mut Bencher) -> Result<()> {
                // Prewarming seems to hurt performance in this case, so we just use `Mode::Fork`
                bench_graalvm_native(bencher, $name, &[$($args,)*], Mode::Fork)
            }

            #[bench]
            fn $teavm(bencher: &mut Bencher) -> Result<()> {
                bench_teavm(bencher, $name, &[$($args,)*])
            }
        )*)
    }

    benchmarks! {
        jvm_direct_mandelbrot jvm_fork_mandelbrot graalvm_native_fork_mandelbrot teavm_mandelbrot "mandelbrot" "200",
        jvm_direct_nbody jvm_fork_nbody graalvm_native_fork_nbody teavm_nbody "nbody" "10000",
        jvm_direct_pidigits jvm_fork_pidigits graalvm_native_fork_pidigits teavm_pidigits "pidigits" "100",
        jvm_direct_spectralnorm jvm_fork_spectralnorm graalvm_native_fork_spectralnorm teavm_spectralnorm "spectralnorm" "100",
        jvm_direct_simple jvm_fork_simple graalvm_native_fork_simple teavm_simple "simple" "200",
        jvm_direct_hello jvm_fork_hello graalvm_native_fork_hello teavm_hello "hello" "hello, world!",
    }
}
