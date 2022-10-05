#![deny(warnings)]
#![feature(test)]

extern crate test;

#[cfg(test)]
mod tests {
    use {
        anyhow::Result,
        jni::{
            objects::{JObject, JValue},
            signature::{JavaType, Primitive},
            InitArgsBuilder, JNIVersion, JavaVM,
        },
        std::env,
        test::Bencher,
    };

    fn bench_jvm(bencher: &mut Bencher, class_name: &str, arguments: &[&str]) -> Result<()> {
        let jvm = JavaVM::new(
            InitArgsBuilder::new()
                .version(JNIVersion::V8)
                .option(&format!("-Djava.class.path={}", env::var("CLASSPATH")?))
                .build()?,
        )?;
        let env = jvm.attach_current_thread()?;
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
        let args = env.new_object_array(
            arguments.len().try_into()?,
            "java/lang/String",
            JObject::null(),
        )?;
        for (index, argument) in arguments.iter().enumerate() {
            env.set_object_array_element(args, index.try_into()?, env.new_string(argument)?)?;
        }

        bencher.iter(|| {
            env.call_static_method_unchecked(
                class,
                method,
                JavaType::Primitive(Primitive::Void),
                &[JValue::Object(JObject::from(args))],
            )
            .unwrap();
        });

        Ok(())
    }

    fn bench_teavm(bencher: &mut Bencher, _class_name: &str, _arguments: &[&str]) -> Result<()> {
        // TODO
        bencher.iter(|| ());

        Ok(())
    }

    macro_rules! benchmarks {
        ($($jvm:ident $teavm:ident $name:literal $($args:literal)*,)*) => ($(
            #[bench]
            fn $jvm(bencher: &mut Bencher) -> Result<()> {
                bench_jvm(bencher, $name, &[$($args,)*])
            }

            #[bench]
            fn $teavm(bencher: &mut Bencher) -> Result<()> {
                bench_teavm(bencher, $name, &[$($args,)*])
            }
        )*)
    }

    benchmarks! {
        jvm_mandelbrot teavm_mandelbrot "mandelbrot" "200",
        jvm_nbody teavm_nbody "nbody" "10000",
        jvm_pidigits teavm_pidigits "pidigits" "10000",
        jvm_spectralnorm teavm_spectralnorm "spectralnorm" "100",
        jvm_simple teavm_simple "simple" "200",
    }
}
