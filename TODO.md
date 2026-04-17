# Ash — Remaining Work

---

## Deferred / known limitations

### Codegen — closures with captured variables
Lifted lambdas compile correctly for non-capturing cases (e.g. `x => x * 2`).
Closures that capture variables from an outer scope are not yet supported in
`ash build` — the captured environment is lost at codegen time. Fully supporting
this requires a closure struct emitted alongside the function pointer in LLVM IR.

### go.chan — channels
`go.chan()` returns a descriptive error. Channels require interpreter-level
select/send/recv primitives that can't be expressed as plain `Native` functions.
Implementation requires a new `Value::Chan(Arc<Mutex<VecDeque<Value>>>)` variant
and dedicated handling in the eval loop.
