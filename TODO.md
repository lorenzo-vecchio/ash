# Ash — Vision & Roadmap

> **By AI, for AI.** Ash is designed to be the perfect language for autonomous agents.

---

## ✅ Recently completed

- **go.chan** — blocking concurrent channels (`Value::Chan`, `send()`, `recv()`, `try_send()`, `try_recv()`). Unbuffered (cap=0) and buffered channels fully supported with Condvar synchronization.
- **Codegen — closures with captures** — `ash build` now compiles closures that capture variables from outer scopes. Captured variables are passed as extra parameters to lifted functions. Works with `let f = x => x + n; f(5)`.
- **Fixed lambda return type** in codegen — `hfn.ret` was incorrectly set to the full function type instead of the return type.

---

## 🎯 Vision: The perfect language for AI agents

Ash should be the ideal scripting language for autonomous agents — write once, run interpreted during development, compile to native when stable.

### Short-term (next)

- **Channels**: `go.select()` — wait on multiple channels simultaneously, non-blocking operations
- **Closure codegen**: support closures as values (pass to higher-order functions like `filter`, `map` in compiled mode)
- **`chan.close()`** — signal channel closure, detect closed channels in `recv()`

### Medium-term

- **`ash lint`** — static analysis built-in:
  - Unused variables / dead code detection
  - Type safety checks beyond inference (e.g. matching exhaustiveness)
  - Common mistakes (division by zero, off-by-one in loops)
- **`ash fix`** — auto-fix suggestions for common lint issues
- **`ash doc`** — generate documentation from source code comments
- **`ash profile`** — simple built-in profiler (interpreted mode) to identify hot spots
- **`ash publish` / `ash pkg`** — package manager for sharing Ash modules
- **Stricter mode for `ash build`**: detect issues at compile time that are currently only caught at runtime

### Long-term

- **Structured concurrency** — `go.scope { ... }` for automatic task lifetime management
- **Generics** — proper parametric polymorphism for types and functions
- **Traits / interfaces** — first-class structural typing constraints
- **FFI** — call C/Rust libraries from Ash without wrappers
- **Macros** — compile-time metaprogramming for zero-cost abstractions
- **Async/await** — non-blocking I/O without threads

---

## 📊 Benchmarking

Benchmark results must be tracked in the repo to measure progress.

### How to run

```bash
./benchmark/run_benchmark.sh              # timing benchmarks
python3 benchmark/token_count.py          # token count benchmarks
```

### Current results

| Language | ms/run (compute) | tokens (CRUD app) |
|----------|-----------------|-------------------|
| Ash (compiled) | 6ms | 334 |
| Go | 8ms | 487 |
| Python | 69ms | 350 |
| Java | 150ms | 578 |

### What to benchmark periodically

1. **Compute**: fib(25) + count_primes(1000) + collatz(27) + sum_range(10000)
2. **Token count**: identical todo CRUD backend across languages
3. **Startup time**: `ash run hello.ash` vs `python3 hello.py` vs `node hello.js`
4. **Binary size**: `ash build` output vs Go static binary vs Java JAR

---

## 🛠 Deferred / known limitations

### go.select — channel multi-wait
`go.select(ch1 ch2)` — wait on multiple channels. Requires a new runtime primitive.
Not yet implemented; single-channel blocking send/recv works.

### AI embedding endpoints
`ai.embed`, `ai.classify`, `ai.moderate` — registered in stdlib but use the
generic Anthropic API endpoint. Dedicated embedding/classification APIs would
be faster and cheaper.

### Codegen — closures as values
Closures assigned to variables and called by name work in `ash build`.
Passing closures as values to higher-order functions (e.g., `filter(list, fn)`)
in compiled mode still needs a proper closure struct representation.

### Codegen — method calls
String/list methods like `s.len()`, `list.map(f)` are not implemented in
compiled mode — they work only in `ash run`.

### LSP — completions and hover
The language server provides basic syntax highlighting and error reporting.
Semantic features (go-to-definition, completions, hover types) are pending.

---

## 💡 Ideas to explore

### For AI agents specifically
- **Structured output mode**: `ai.structured("Extract name and age", {name: str, age: int})` → typed result
- **Tool calling**: first-class `Tool` type with schema generation from function signatures
- **Approval gates**: `await user.approve("Delete file {path}?")` — block until human confirms
- **State persistence**: built-in key-value store with automatic serialization (`state.get`, `state.set`)
- **Sandboxing**: `sandbox.run(code)` — execute untrusted Ash in a restricted sub-interpreter

### Performance
- **JIT compilation** for `ash run` via Cranelift or similar
- **Lazy evaluation** of pipeline expressions to avoid intermediate allocations
- **Arena allocation** for short-lived interpreter values

### Developer experience
- **`ash init`** — scaffold a new Ash project with recommended structure
- **`ash build --release`** — strip debug info, optimize for size/speed
- **`ash build --target wasm`** — compile to WebAssembly
- **`ash fmt`** with `--check` for CI integration (already exists!)
- **Rich error messages with suggested fixes** ("did you mean `println` instead of `printn`?")
