# Ash Language — Remaining Work

All initial 26 implementation items are complete. This file tracks what is still missing
before Ash is fully production-ready.

---

## Remaining / nice-to-have

### go.spawn — real implementation
`go.spawn` is currently a stub that returns an error. Proper implementation requires
making `Value: Send + Sync` (audit `FnBody::Native` and `Env`) then using
`std::thread::spawn` + `Arc<Mutex<Option<Value>>>` for the task handle.
`go.all`, `go.race`, and `go.wait` can follow once spawn works.

### auth.* — real implementation
Currently stubs. Needs:
- `jsonwebtoken` crate for `auth.jwt` / `auth.verify`
- `bcrypt` crate for `auth.hash` / `auth.check`

### mail.* — real implementation
Currently a stub. Needs `lettre` crate + SMTP config via `SMTP_HOST`, `SMTP_USER`,
`SMTP_PASS` env vars.

### store.* — real implementation
Currently a stub. Implement local filesystem backend first, then S3-compatible via
`STORE_URL` env var.

### Codegen: map and tuple literals
`HirExprKind::Map` and tuple expressions return a codegen error with a suggestion to use
`ash run`. A C runtime helper `ash_map_*` is needed (similar to `ash_list_*`).

### Package manager
No `ash.toml` format exists. Define the format and add an `ash add` command that fetches
packages from git URLs or a future registry.

### ash check — real type error reporting
`ash check` currently just counts fns/types/stmts. It should surface typechecker
diagnostics (type mismatches, undefined variables) with line numbers.

### ash run --strict
Optionally run the typechecker before interpretation to catch type errors at
`ash run` time, not just at `ash build` time.
