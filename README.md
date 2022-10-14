# java-faas-benchmarks

This repo includes a harness and small set of benchmarks for comparing
performance between OpenJDK and TeaVM/Wasmtime in a multi-tenant
fuction-as-a-service (FaaS) environment.  In such an environment, each
invocation of a function must be isolated from others to ensure security and
statelessness.

Each case is benchmarked using three different strategies:

- Using OpenJDK's [JNI Invocation API](https://docs.oracle.com/en/java/javase/18/docs/specs/jni/invocation.html).  Each invocation reuses the same JVM.

- As above, but forking a new process for each invocation.

- Using [TeaVM-WASI](https://github.com/fermyon/teavm-wasi) to target WebAssembly, which is run in [Wasmtime](https://github.com/bytecodealliance/wasmtime).  Each invocation uses a new [module instance](https://docs.rs/wasmtime/latest/wasmtime/struct.Instance.html).

## Running the benchmarks

### Prerequisites

- POSIX-compatible OS (due to the use of the `fork` system call)
- [Rust](https://rustup.rs/)
- A recent OpenJDK (e.g. `apt install openjdk-18-jdk-headless`)
- GraalVM (I used GraalVM CE 21.3.3.1)
- Maven (e.g. `apt install maven`)
- (Optional) `wasm-opt` (e.g. `apt install binaryen`)

### Build the test cases

Adjust `GRAALVM_HOME` according to where you've installed it.

```
export GRAALVM_HOME=/opt/graalvm-ce-java17-21.3.3.1
export JAVA_HOME=$GRAALVM_HOME
export PATH=$GRAALVM_HOME/bin:$PATH
(cd apps && mvn -Pnative -DskipTests package)
# optionally run `wasm-opt`:
for x in mandelbrot nbody pidigits spectralnorm simple hello; do
  wasm-opt -O3 -s 0 --strip apps/$x/target/generated/wasm/teavm-wasm/classes.wasm -o $x.wasm
  mv $x.wasm apps/$x/target/generated/wasm/teavm-wasm/classes.wasm
done
```

### Run the benchmarks

Adjust `LD_LIBRARY_PATH` according to where libjvm.so is in your OpenJDK installation.

```
LD_LIBRARY_PATH=/usr/lib/jvm/java-18-openjdk-amd64/lib/server/ cargo +nightly bench
```

### Sample results (Ubuntu 22.04, OpenJDK 18, Intel(R) Core(TM) i7-7600U CPU @ 2.80GHz, 16GB RAM)

```
test tests::graalvm_native_fork_hello        ... bench:   1,610,096 ns/iter (+/- 273,953)
test tests::graalvm_native_fork_mandelbrot   ... bench:   5,727,398 ns/iter (+/- 710,769)
test tests::graalvm_native_fork_nbody        ... bench:   3,537,420 ns/iter (+/- 603,914)
test tests::graalvm_native_fork_pidigits     ... bench:   3,486,886 ns/iter (+/- 429,200)
test tests::graalvm_native_fork_simple       ... bench:   6,293,284 ns/iter (+/- 706,912)
test tests::graalvm_native_fork_spectralnorm ... bench:   3,624,850 ns/iter (+/- 765,348)

test tests::jvm_direct_hello                 ... bench:         834 ns/iter (+/- 3,151)
test tests::jvm_direct_mandelbrot            ... bench:   3,498,878 ns/iter (+/- 78,412)
test tests::jvm_direct_nbody                 ... bench:     984,568 ns/iter (+/- 44,228)
test tests::jvm_direct_pidigits              ... bench:     559,801 ns/iter (+/- 119,735)
test tests::jvm_direct_simple                ... bench:   3,622,829 ns/iter (+/- 70,974)
test tests::jvm_direct_spectralnorm          ... bench:   1,439,085 ns/iter (+/- 76,239)

test tests::jvm_fork_hello                   ... bench:   2,120,149 ns/iter (+/- 446,394)
test tests::jvm_fork_mandelbrot              ... bench:  36,616,057 ns/iter (+/- 13,863,304)
test tests::jvm_fork_nbody                   ... bench:  48,834,242 ns/iter (+/- 12,484,918)
test tests::jvm_fork_pidigits                ... bench:  24,822,119 ns/iter (+/- 7,837,427)
test tests::jvm_fork_simple                  ... bench:  47,904,725 ns/iter (+/- 14,330,855)
test tests::jvm_fork_spectralnorm            ... bench:  30,455,109 ns/iter (+/- 10,006,388)

test tests::teavm_hello                      ... bench:      69,947 ns/iter (+/- 6,950)
test tests::teavm_mandelbrot                 ... bench:   3,476,083 ns/iter (+/- 285,846)
test tests::teavm_nbody                      ... bench:   4,899,842 ns/iter (+/- 564,920)
test tests::teavm_pidigits                   ... bench:   3,469,015 ns/iter (+/- 430,097)
test tests::teavm_simple                     ... bench:   7,162,484 ns/iter (+/- 674,441)
test tests::teavm_spectralnorm               ... bench:   1,758,527 ns/iter (+/- 293,928)
```

### Known Issue(s)

- The `jvm_fork_spectralnorm` test occasionally hangs forever.  If this happens, Ctrl-C and re-run the test.
