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
- Maven (e.g. `apt install maven`)
- (Optional) `wasm-opt` (e.g. `apt install binaryen`)

### Build the test cases

```
(cd apps && mvn prepare-package)
# optionally run `wasm-opt`:
for x in mandelbrot nbody pidigits spectralnorm simple hello; do
  wasm-opt -O3 -s 0 --strip apps/$x/target/generated/wasm/teavm-wasm/classes.wasm -o $x.wasm
  mv $x.wasm apps/$x/target/generated/wasm/teavm-wasm/classes.wasm
done
```

### Run the benchmarks

```
LD_LIBRARY_PATH=/usr/lib/jvm/java-18-openjdk-amd64/lib/server/ cargo +nightly bench
```

### Sample results (Intel(R) Core(TM) i7-7600U CPU @ 2.80GHz, 16GB RAM)

```
test tests::jvm_fork_hello        ... bench:   2,185,777 ns/iter (+/- 130,437)
test tests::jvm_fork_mandelbrot   ... bench:  66,051,657 ns/iter (+/- 23,271,194)
test tests::jvm_fork_nbody        ... bench:  61,235,858 ns/iter (+/- 11,372,554)
test tests::jvm_fork_pidigits     ... bench:  28,600,132 ns/iter (+/- 7,590,476)
test tests::jvm_fork_simple       ... bench:  58,974,509 ns/iter (+/- 12,717,177)
test tests::jvm_fork_spectralnorm ... bench:  37,914,462 ns/iter (+/- 9,183,140)

test tests::jvm_hello             ... bench:         858 ns/iter (+/- 228)
test tests::jvm_mandelbrot        ... bench:   3,511,887 ns/iter (+/- 22,645)
test tests::jvm_nbody             ... bench:   1,000,819 ns/iter (+/- 47,021)
test tests::jvm_pidigits          ... bench:     574,438 ns/iter (+/- 66,105)
test tests::jvm_simple            ... bench:   3,623,500 ns/iter (+/- 42,554)
test tests::jvm_spectralnorm      ... bench:   1,433,495 ns/iter (+/- 25,555)

test tests::teavm_hello           ... bench:      78,262 ns/iter (+/- 10,996)
test tests::teavm_mandelbrot      ... bench:   3,650,497 ns/iter (+/- 70,290)
test tests::teavm_nbody           ... bench:   5,508,365 ns/iter (+/- 715,595)
test tests::teavm_pidigits        ... bench:   3,880,315 ns/iter (+/- 611,833)
test tests::teavm_simple          ... bench:   7,755,494 ns/iter (+/- 1,206,601)
test tests::teavm_spectralnorm    ... bench:   1,784,634 ns/iter (+/- 98,286)
```

### Known Issue(s)

- The `jvm_fork_spectralnorm` test occasionally hangs forever.  If this happens, Ctrl-C and re-run the test.
