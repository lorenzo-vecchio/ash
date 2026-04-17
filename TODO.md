# Ash — Remaining Work

Priority order: compiler/interpreter → CLI → stdlib → package manager.

---

## 1. Compiler & Interpreter

### 1.1 go.spawn — real implementation (interpreter)
`go.spawn` is a stub. Requires making `Value: Send + Sync`:
- Audit `FnBody::Native`: change `Arc<dyn Fn>` to `Arc<dyn Fn + Send + Sync>`
- Audit `Env`: globals are already `Arc<Mutex<...>>` — should be fine
- Implement with `std::thread::spawn`, wrap result in `Value::Task(Arc<Mutex<Option<Value>>>)`
- `go.wait(task)` — block on the mutex until `Some`
- `go.all(tasks)` — join all task threads
- `go.race(tasks)` — poll in a loop, return first `Some`
- `go.chan()` — `std::sync::mpsc::channel`, expose as `Value::Chan`

### 1.2 Codegen — map literals
`HirExprKind::Map` returns a codegen error. Add a C runtime helper `ash_map_*` to
`ash_runtime.c` (string-keyed, `Value*`-valued hash map), declare it in codegen, and
emit `ash_map_new` + `ash_map_set` calls for each key-value pair.

### 1.3 Codegen — closures / lambdas as values
Lambdas passed as arguments fail in compiled mode. Requires:
- Lifting lambdas to top-level named functions in the HIR (capture list becomes extra params)
- Emitting a function pointer + closure struct in LLVM IR
- Updating call sites that receive a lambda to indirect-call through the pointer

### 1.4 Codegen — stdlib calls beyond math.* and println
Arithmetic programs compile fine but any stdlib call (file, http, json, db, etc.) is
missing. Fix: emit `declare` lines for each stdlib function and link against a
`ash_stdlib.c` that wraps the Rust implementations via `extern "C"`.

### 1.5 ash run --strict
Add `--strict` flag to `ash run` that runs the typechecker before interpretation and
surfaces type mismatch errors with line numbers. Lets users catch errors before deploying
without needing to go through `ash build`.

---

## 2. CLI

### 2.1 ash check — real type error reporting
Currently always prints `OK (N fns, M types, K stmts)` even for broken programs.
- Thread diagnostics out of `ash-typeck` as `Vec<Diagnostic { msg, line, col }>`
- Print each error with file + line, matching the format of runtime errors
- Exit with code 1 if any error is found
- Add `ash check --watch file.ash` that re-checks on file save (use `notify` crate)

---

## 3. Stdlib

### 3.1 HTTP server — http.serve / http.listen
The most impactful missing primitive. Without it Ash can't be used for any server-side
application. Suggested API:

```ash
http.serve(8080, req =>
    if req.path == "/hello"
        { status: 200, body: "hello world" }
    else
        { status: 404, body: "not found" }
)
```

Implementation:
- Add `Value::Request { method, path, headers, body }` and `Value::Response { status, headers, body }`
- Use `tiny_http` crate (zero-dependency, synchronous) or `hyper` (async, higher performance)
- `http.serve(port, handler_fn)` — blocking; each request calls the Ash handler function
- `http.router()` — optional helper that returns a dispatch table (map of path → handler)

### 3.2 auth.* — real implementation
Replace stubs with:
- `auth.hash(password)` / `auth.check(password, hash)` — `bcrypt` crate
- `auth.jwt(payload, secret)` — `jsonwebtoken` crate, HS256 by default
- `auth.verify(token, secret)` — decode and return payload as `Value::Map`, or `none` on failure

### 3.3 mail.* — real implementation
Replace stub with `lettre` crate:
- Read `SMTP_HOST`, `SMTP_PORT`, `SMTP_USER`, `SMTP_PASS` from env
- `mail.send(to, subject, body)` — plaintext
- `mail.html(to, subject, html)` — HTML body

### 3.4 store.* — real implementation
Two-stage:
1. Local filesystem backend (always available, no config needed):
   `store.put(key, data)` writes to `~/.ash/store/<key>`, `store.url` returns a `file://` path
2. S3-compatible backend when `STORE_URL=s3://bucket@region` is set — use `aws-sdk-s3` or
   the lighter `s3` crate

---

## 4. Package Manager

### 4.1 ash.toml format
Define the project manifest:

```toml
[package]
name    = "my-app"
version = "0.1.0"
entry   = "main.ash"

[dependencies]
ash-http = { git = "https://github.com/someone/ash-http", tag = "v0.2.0" }
utils    = { path = "../utils" }
```

### 4.2 ash add / ash install
- `ash add <url-or-name>` — fetch package, add to `ash.toml`, write lockfile `ash.lock`
- `ash install` — restore all dependencies from `ash.lock` into `.ash/packages/`
- Packages are plain `.ash` files (or a directory with a `ash.toml`); no binary blobs

### 4.3 Folder-based module auto-discovery
Remove the need for explicit `use` statements:
- At program start, scan the project directory tree for `.ash` files
- Build a module namespace map: `models/user.ash` → `models.User` is in scope
- Circular dependencies → compile-time error with the import cycle printed
