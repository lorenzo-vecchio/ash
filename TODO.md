# Ash Language — Implementation TODO

This document is a complete, standalone reference for someone picking up the Ash
codebase to continue implementation. It describes every gap between what is
specified and what works, where the relevant code lives, and exactly what needs
to change.

---

## Project layout

```
ash/
├── Cargo.toml                     workspace root
├── crates/
│   ├── ash-lexer/src/lib.rs       ~740 lines  — tokenizer, indent/dedent
│   ├── ash-parser/src/lib.rs      ~853 lines  — recursive-descent parser
│   ├── ash-parser/src/ast.rs                  — AST node types
│   ├── ash-hir/src/lib.rs         ~271 lines  — typed HIR + TypeEnv + TypeRegistry
│   ├── ash-typeck/src/lib.rs     ~1099 lines  — type inference, AST→HIR lowering
│   ├── ash-interp/src/lib.rs     ~1717 lines  — tree-walking interpreter
│   ├── ash-codegen/src/lib.rs     ~772 lines  — LLVM IR emitter
│   └── ash-stdlib/src/lib.rs      ~612 lines  — stdlib function descriptors (docs only)
└── ash-cli/src/main.rs            ~444 lines  — CLI: run/build/check/fmt/docs/repl
```

The pipeline is:
  source → Lexer → Parser → AST → (TypeChecker → HIR) → Interpreter  (ash run)
  source → Lexer → Parser → AST → TypeChecker → HIR → Codegen → LLVM IR → clang → binary  (ash build)

The interpreter (`ash run`) does NOT go through the typechecker — it walks the AST
directly. The typechecker only feeds the codegen path.

---

## What actually works today

Test these yourself — they all produce correct output:

**Interpreter (ash run):**
- All arithmetic: `+`, `-`, `*`, `/`, `%`, unary `-`, `!`, `&&`, `||`
- All comparisons: `==`, `!=`, `<`, `>`, `<=`, `>=`
- Variable bindings: `x = 5`, `let x = 5`, `mut x = 5`, `let x: int = 5`
- String interpolation with identifiers: `"hello {name}"`
- String interpolation with expressions: `"result is {x + 1}"`
- String concatenation: `"hello" + " " + "world"`
- String methods: `.len()`, `.upper()`, `.lower()`, `.trim()`, `.split(sep)`,
  `.contains(s)`, `.starts(s)`, `.ends(s)`, `.find(s)`, `.replace(a, b)`
- List literals: `[1, 2, 3]`
- List methods: `.len()`, `.push(x)`, `.first()`, `.last()`, `.reverse()`,
  `.sort()`, `.contains(x)`, `.filter(f)`, `.map(f)`
- List index read: `items[0]` returns `some(val)` or `none`
- List index write (mutation): `items[0] = 99` ✓
- Struct type definition: `type Point\n    x: int\n    y: int`
- Struct literal: `Point { x: 1, y: 2 }`
- Struct field read: `p.x`
- Struct field write: `p.x = 99`
- Union type definition: `type Shape = Circle(float) | Rect(float float)`
- Union construction: `Circle(5.0)`, `Rect(3.0, 4.0)`
- Pattern matching on union variants, literals, wildcards, tuples, struct patterns (some)
- `match Some(v)`, `match None` — works via Pattern::Variant matching Value::Option
- `match Ok(v)`, `match Err(e)` — works when user defines their own Result union type
- Functions: `fn f(a b)\n    a + b`
- Functions with type annotations: `fn f(a:int b:int):int\n    a+b`
- Recursive functions
- Nested functions
- Early return: `return expr`
- Lambdas: `x => x + 1`, `(x y) => x + y`
- Closures (lexical capture of outer scope variables)
- Higher-order core builtins: `filter(list, fn)`, `map(list, fn)`,
  `reduce(list, fn, init)`, `zip(a, b)`, `flat(list)`, `any(list, fn)`, `all(list, fn)`
- Core builtins: `print`, `println`, `abs`, `min`, `max`, `clamp`, `int`, `float`,
  `str`, `bool`, `panic`
- `math.*`: `floor`, `ceil`, `round`, `sqrt`, `pow`, `log`, `log2`, `log10`,
  `sin`, `cos`, `tan`, `pi`, `e`
- `file.*`: `read`, `write`, `exists`, `ls`, `rm`, `mkdir`
- `env.*`: `get`, `require`
- `if / else if / else` as both statement and expression
- `while` loops
- `for x in list` loops
- Null coalescing: `x ?? default`
- Safe navigation: `x?.field`
- Pipelines on a single line: `items |> filter(x => x > 0) |> map(x => x * 2)`
- Global variable mutation from inside functions (via Arc<Mutex> shared globals)
- `ash build` for: arithmetic, recursion, control flow, `while`, match, `math.*`,
  `println` with int/float/str args, functions with explicit type annotations

**CLI:**
- `ash run file.ash` — interpret and run
- `ash build file.ash -o out` — compile via LLVM IR + clang
- `ash check file.ash` — parse + typecheck (reports fn/type/stmt counts)
- `ash fmt file.ash` — basic indent normalization
- `ash docs [namespace]` — show stdlib function signatures
- `ash repl` — interactive REPL with state persistence across lines

---

## BUG 1 — `none`, `Some`, `Ok`, `Err` not defined as global names

**Symptom:**
```
x = none          → runtime: undefined variable 'none'
x = Some(42)      → runtime: undefined variable 'Some'
x = Ok(42)        → runtime: undefined variable 'Ok'
x = Err("oops")   → runtime: undefined variable 'Err'
```

**Root cause:**
The interpreter has `Value::Option(None)`, `Value::Option(Some(box))`, `Value::Ok(box)`,
`Value::Err(box)` as internal Rust variants, but never registers `none`, `Some`, `Ok`,
`Err` as callable names in the global environment.

Pattern matching on `Some(v)` / `None` / `Ok(v)` / `Err(e)` WORKS when the user has
defined their own union type (e.g. `type Result = Ok(int) | Err(str)`) because those
constructors ARE registered at type definition time. The built-in Option/Result values
have no such registration.

**Fix — file: `crates/ash-interp/src/lib.rs`, function: `register_stdlib()`**

Add to `register_stdlib()`:

```rust
// none — the absent Option value
self.define("none", Value::Option(None));

// Some(x) — wrap a value in Option
self.define("Some", Value::Fn(FnValue {
    name: Some("Some".into()), params: vec![],
    body: FnBody::Native(Arc::new(|args| {
        let v = args.into_iter().next().unwrap_or(Value::Unit);
        Ok(Value::Option(Some(Box::new(v))))
    })),
    closure: Env::default(),
}));

// None — alias for none (capitalized form for pattern matching consistency)
self.define("None", Value::Option(None));

// Ok(x) — built-in Result Ok
self.define("Ok", Value::Fn(FnValue {
    name: Some("Ok".into()), params: vec![],
    body: FnBody::Native(Arc::new(|args| {
        let v = args.into_iter().next().unwrap_or(Value::Unit);
        Ok(Value::Ok(Box::new(v)))
    })),
    closure: Env::default(),
}));

// Err(x) — built-in Result Err
self.define("Err", Value::Fn(FnValue {
    name: Some("Err".into()), params: vec![],
    body: FnBody::Native(Arc::new(|args| {
        let v = args.into_iter().next().unwrap_or(Value::Unit);
        Ok(Value::Err(Box::new(v)))
    })),
    closure: Env::default(),
}));
```

---

## BUG 2 — `!` error propagation is broken

**Symptom:**
```ash
type Result = Ok(int) | Err(str)
fn risky()
    Err("broke")
fn caller()
    v = risky()!       # should short-circuit out of caller with Err
    println("got {v}") # this line executes anyway — "got Err(broke)"
```
The `!` operator is silently a no-op on `Value::Variant` values (user-defined union
variants). It only works on `Value::Ok` / `Value::Err` — but those are only produced
by the built-in Ok/Err (which aren't registered — see Bug 1).

Even when using the built-in `Value::Err`, there is a second bug: the `Propagated`
error kind is raised but the calling function's exec loop doesn't catch it and convert
it into a `Value::Err` return. It propagates all the way up and crashes the program
instead of being caught at the function boundary.

**Root cause — `crates/ash-interp/src/lib.rs`:**

1. `ExprKind::Propagate` at line ~848 only matches `Value::Err(e)` and `Value::Ok(v)`.
   User-defined `Value::Variant("Err", ...)` is not matched.

2. The `FnBody::Ast` execution loop at line ~1223 catches `ErrorKind::Return` but not
   `ErrorKind::Propagated`:
   ```rust
   Err(e) if e.kind == ErrorKind::Return => { ... }
   Err(e) => { self.env.pop(); self.env.locals = saved_locals; return Err(e); }
   ```
   `Propagated` falls through to the final arm and returns the error raw instead of
   converting it to `Value::Err(...)`.

**Fix — two parts:**

Part A: in `ExprKind::Propagate`, also match user-defined `Err` variants:
```rust
ExprKind::Propagate(expr) => {
    let v = self.eval_expr(expr)?;
    match v {
        Value::Err(e)  => Err(InterpError { kind: ErrorKind::Propagated, msg: e.to_string() }),
        Value::Ok(v)   => Ok(*v),
        // User-defined Err-named variant
        Value::Variant(ref name, ref fields) if name == "Err" => {
            let msg = fields.first().map(|v| v.to_string()).unwrap_or_default();
            Err(InterpError { kind: ErrorKind::Propagated, msg })
        }
        // Option None propagation (? operator semantics)
        Value::Option(None) => Err(InterpError { kind: ErrorKind::Propagated, msg: "none".into() }),
        Value::Option(Some(v)) => Ok(*v),
        other => Ok(other),
    }
}
```

Part B: in `FnBody::Ast` execution loop, catch `Propagated` and wrap it as `Value::Err`:
```rust
Err(e) if e.kind == ErrorKind::Propagated => {
    self.env.pop();
    self.env.locals = saved_locals;
    // Return the propagated error as a Value::Err so the caller can handle it
    return Ok(Value::Err(Box::new(Value::Str(e.msg))));
}
```

---

## BUG 3 — Map literals don't parse

**Symptom:**
```
m = {"a": 1, "b": 2}   → error[parse]: unexpected token '{' in expression
m = {}                  → error[parse]: unexpected token '{' in expression
```

**Root cause — `crates/ash-parser/src/lib.rs`, `fn parse_primary()`:**

`parse_primary()` has no arm for `Token::LBrace`. When the parser sees `{` in expression
position, it falls through to the error case. The AST already has `ExprKind::Map` and
the interpreter already handles `Value::Map` — the only missing piece is the parser arm.

The complication: `{` also starts indented blocks (function bodies etc.) — but those
always follow a keyword or `:` on the previous line, never appear after `=` on the same
line as a standalone expression. The disambiguation rule: `{` in expression position on
the same line → map literal; `{` after a newline following a block-starting construct
→ handled by the indent/dedent system (not as an expression).

**Fix — `crates/ash-parser/src/lib.rs`, add to `parse_primary()` before the final `_` arm:**

```rust
Token::LBrace => {
    self.advance();
    let mut pairs = vec![];
    while self.peek() != &Token::RBrace && !self.at_eof() {
        self.skip_newlines();
        if self.peek() == &Token::RBrace { break; }
        let key = self.parse_expr()?;
        self.expect(&Token::Colon)?;
        let val = self.parse_expr()?;
        pairs.push((key, val));
        self.eat(&Token::Comma);
        self.skip_newlines();
    }
    self.expect(&Token::RBrace)?;
    Ok(Expr { kind: ExprKind::Map(pairs), span })
}
```

The interpreter already evaluates `ExprKind::Map` correctly (line ~728 in ash-interp).
The HIR lowerer already lowers `ExprKind::Map` (check ash-typeck). Nothing else to change.

---

## BUG 4 — `fmt()` and `{}` in string interpolation crash

**Symptom:**
```
fmt("hello {}", "world")       → runtime: undefined variable '' in string interpolation
println("result: {}")          → runtime: undefined variable '' in string interpolation
```

**Root cause — `crates/ash-interp/src/lib.rs`, `fn interpolate()`:**

When `{}` contains nothing (empty string after trim), the fast path
`expr_src.chars().all(|c| c.is_alphanumeric() || c == '_')` returns `true` for an empty
string. Then `self.env.get("")` fails with `undefined variable ''`.

The fix is trivial: handle the empty case explicitly.

**Fix — `fn interpolate()`, add at the start of the `else { // Arbitrary expression }` branch:**

```rust
let expr_src = expr_src.trim().to_string();

// Empty braces {} — used by fmt() as positional placeholder, skip silently
if expr_src.is_empty() {
    result.push_str("{}"); // leave as-is for fmt() to fill, or push placeholder
    continue;
}
```

Actually the better fix depends on how `fmt()` is meant to work. Two options:

**Option A:** `fmt("hello {}", "world")` — the `{}` in the template string should be left
as a literal `{}` during string parsing (not treated as interpolation), and `fmt()` replaces
them positionally with its extra args. This requires the lexer to NOT expand `{}` inside
strings that are arguments to `fmt()` — complex.

**Option B (simpler):** Treat `{}` as a positional placeholder marker `\x00` or similar
during lexing of string literals when they contain only `{}`. The `fmt()` function then
does a string replace of `{}` with its args in order.

**Recommended fix:** Make `fmt()` handle the `{}` placeholder entirely in Rust, bypassing
the `interpolate()` call. The `fmt()` builtin already works via `result.replacen("{}", ...)`.
The issue is that the string `"hello {}"` gets interpolated BEFORE it reaches `fmt()`,
eating the `{}`. Fix: The lexer should not interpolate `{}` (empty braces) — it should
leave them as the two-character sequence `{}` in the string value.

In `fn interpolate()`:
```rust
if expr_src.is_empty() {
    result.push('{');
    result.push('}');
} else if expr_src.chars().all(|c| c.is_alphanumeric() || c == '_') {
    // ... existing fast path
}
```

---

## BUG 5 — Range syntax `0..n` doesn't parse

**Symptom:**
```
for i in 0..5     → error[parse]: expected '<INDENT>', got '..'
r = 0..10         → error[parse]: unexpected token '..' in expression
```

**Root cause:**
`Token::DotDot` exists in the lexer (line ~50 in ash-lexer), and `ExprKind::Range` exists
in the AST, and the interpreter handles `ExprKind::Range` (line ~858 in ash-interp). But
the parser never consumes `DotDot` tokens — there is no `parse_range()` call and no
`DotDot` arm in `parse_primary()` or `parse_binop()`.

**Fix — `crates/ash-parser/src/lib.rs`:**

Ranges are best parsed as a binary-operator-level construct (like `+`) so that `a + b..c`
parses as `a + (b..c)` — or alternatively in `parse_primary` when the left side is an
integer. The cleanest approach is to handle `..` as a low-precedence binary operator in
`parse_binop()`:

```rust
// After parsing the left side, check for ..
if self.eat(&Token::DotDot) {
    let rhs = self.parse_binop()?;
    return Ok(Expr {
        kind: ExprKind::Range { start: Box::new(lhs), end: Box::new(rhs) },
        span,
    });
}
```

The interpreter already evaluates `ExprKind::Range` by creating a `Value::List` of integers.
For `for i in 0..5`, the `For` statement already iterates over a `Value::List`.

---

## BUG 6 — Multi-line pipeline chains don't parse

**Symptom:**
```ash
result = [1,2,3]
    |> filter(x => x > 0)    → error[parse]: unexpected token '<INDENT>' in expression
    |> map(x => x * 2)
```

Single-line pipelines work: `[1,2,3] |> filter(x => x > 0) |> map(x => x * 2)`

**Root cause — `crates/ash-parser/src/lib.rs`:**

After parsing `result = [1,2,3]`, the parser sees a newline and considers the statement
complete. The next line starting with `|>` gets an INDENT token first, which the parser
doesn't expect in expression position.

**Fix — `crates/ash-parser/src/lib.rs`:**

In `parse_binop()` or `parse_postfix()`, after parsing a full expression, peek ahead
past any INDENT/DEDENT/NEWLINE tokens to check if the next meaningful token is `Pipe`
(`|>`). If so, consume the whitespace and continue the pipeline chain:

```rust
// Allow |> to continue across lines
loop {
    self.skip_newlines_and_indents(); // consume NEWLINE/INDENT/DEDENT
    if self.peek() == &Token::Pipe {
        self.advance();
        let rhs = self.parse_postfix()?; // parse the right side (a function call)
        lhs = Expr { kind: ExprKind::Pipe { lhs: Box::new(lhs), rhs: Box::new(rhs) }, span };
    } else {
        break;
    }
}
```

The key is that `|>` at the start of an indented continuation line should be treated as
continuing the previous expression, not as a new statement. A `skip_newlines_and_indents()`
helper that saves/restores position on failure would allow safe lookahead.

---

## BUG 7 — Type annotation on bare assignment doesn't parse

**Symptom:**
```
x: int = 5     → error[parse]: unexpected token ':' in expression
```
`let x: int = 5` works. `x: int = 5` does not.

**Root cause — `crates/ash-parser/src/lib.rs`, `fn parse_stmt()`:**

The statement parser tries to parse the left side of an assignment as an expression
first. `x` is parsed as an identifier, then `:` is unexpected because it's not a
recognized binary operator.

**Fix — in `parse_stmt()`, when parsing `Ident` followed by `:`, treat it as a typed binding:**

```rust
// Check for annotated assignment: ident: Type = expr
if let Token::Ident(name) = self.peek().clone() {
    if self.peek_at(1) == &Token::Colon {
        let name = self.expect_ident()?;
        self.expect(&Token::Colon)?;
        let ty = self.parse_type()?;
        self.expect(&Token::Assign)?;
        let value = self.parse_expr()?;
        return Ok(Stmt { kind: StmtKind::Let { name, ty, mutable: false, value }, span });
    }
}
```

---

## BUG 8 — Struct pattern in match doesn't parse

**Symptom:**
```ash
type Point
    x: int
    y: int
p = Point { x: 3, y: 4 }
match p
    Point { x: 0, y: 0 } => println("origin")   → error[parse]: expected '=>', got '{'
```

**Root cause — `crates/ash-parser/src/lib.rs`, `fn parse_pattern()`:**

`parse_pattern()` handles `Pattern::Variant(name, inner_pats)` (e.g. `Circle(r)`) but
not `Pattern::Struct(name, field_pats)` (e.g. `Point { x, y }`). When it sees
`Point` followed by `{` it stops after the ident and returns `Pattern::Ident("Point")`,
then the `{` is unexpected.

**Fix — in `parse_pattern()`, when an Ident is followed by `{`:**

```rust
Token::Ident(_) => {
    let name = self.expect_ident()?;
    if self.peek() == &Token::LParen {
        // Variant pattern: Circle(r)
        // ... existing code
    } else if self.peek() == &Token::LBrace {
        // Struct pattern: Point { x, y } or Point { x: 0, y: 0 }
        self.advance();
        let mut fields = vec![];
        while self.peek() != &Token::RBrace && !self.at_eof() {
            let field_name = self.expect_ident()?;
            let pat = if self.eat(&Token::Colon) {
                self.parse_pattern()?
            } else {
                Pattern::Ident(field_name.clone()) // shorthand: Point { x } = Point { x: x }
            };
            fields.push((field_name, pat));
            self.eat(&Token::Comma);
        }
        self.expect(&Token::RBrace)?;
        Pattern::Struct(name, fields)
    } else {
        Pattern::Ident(name)
    }
}
```

---

## BUG 9 — Tuple pattern in match doesn't parse

**Symptom:**
```ash
match pair
    (a, b) => println("{a} {b}")   → error[parse]: expected pattern, got '('
```

**Root cause — `crates/ash-parser/src/lib.rs`, `fn parse_pattern()`:**

`parse_pattern()` has no arm for `Token::LParen`. Tuple patterns like `(a, b)` are
simply not recognized.

**Fix — add to `parse_pattern()`:**

```rust
Token::LParen => {
    self.advance();
    let mut pats = vec![];
    while self.peek() != &Token::RParen && !self.at_eof() {
        pats.push(self.parse_pattern()?);
        self.eat(&Token::Comma);
    }
    self.expect(&Token::RParen)?;
    Pattern::Tuple(pats)
}
```

The interpreter already handles `Pattern::Tuple` (line ~1359 in ash-interp).

---

## BUG 10 — `re.match` keyword conflict

**Symptom:**
```
println(re.match("[0-9]+", "abc123"))   → error[parse]: expected identifier, got 'match'
```

**Root cause:**
`match` is a reserved keyword. When the parser sees `re.match(...)`, after consuming
`re.` it expects an identifier but sees the `Match` token instead.

**Fix — `crates/ash-parser/src/lib.rs`, in `fn parse_postfix()`:**

After parsing `obj.`, allow reserved keywords as field names in this context:

```rust
Token::Dot => {
    self.advance();
    // Allow reserved keywords as method names (e.g. re.match, map.get)
    let field = self.expect_ident_or_keyword()?; // new helper
    // ...
}
```

Add a helper:
```rust
fn expect_ident_or_keyword(&mut self) -> PResult<String> {
    match self.peek().clone() {
        Token::Ident(s) => { self.advance(); Ok(s) }
        // Allow these keywords as method names after a dot
        Token::Match  => { self.advance(); Ok("match".into()) }
        Token::For    => { self.advance(); Ok("for".into()) }
        Token::In     => { self.advance(); Ok("in".into()) }
        _ => Err(self.err(format!("expected identifier, got '{}'", self.peek())))
    }
}
```

---

## MISSING — Unregistered stdlib namespaces

All of these produce `runtime: undefined variable 'X'` because they are never registered
in `register_stdlib()` in `crates/ash-interp/src/lib.rs`. The `ash-stdlib` crate only
contains their type signatures as documentation — no Rust implementations exist.

The fix for each is to add `self.define("namespace.fn", Value::Fn(...))` calls in
`register_stdlib()`. Until a real implementation is ready, at minimum register stub
functions that return a descriptive error:

```rust
self.define("http.get", Value::Fn(FnValue {
    name: Some("http.get".into()), params: vec![],
    body: FnBody::Native(Arc::new(|_| {
        Err(InterpError::runtime("http.get: not yet implemented — add reqwest to ash-interp dependencies"))
    })),
    closure: Env::default(),
}));
```

### `json.*`

Currently `json.str` is registered but only calls `.to_string()` on the value — no real
JSON encoding. `json.parse` is not registered at all.

**Recommended crate:** `serde_json`

Functions to implement:
- `json.parse(s: str) -> ?any` — parse JSON string, return `Value::Option(None)` on error
- `json.str(x: any) -> str` — serialize any `Value` to JSON (needs recursive serializer)
- `json.pretty(x: any) -> str` — pretty-printed JSON

The hardest part is `json.str` — it needs to recursively convert `Value` to a
`serde_json::Value`. A mapping:
- `Value::Int(n)` → `json::Value::Number(n)`
- `Value::Float(f)` → `json::Value::Number(f)`
- `Value::Bool(b)` → `json::Value::Bool(b)`
- `Value::Str(s)` → `json::Value::String(s)`
- `Value::List(items)` → `json::Value::Array([...])`
- `Value::Map(pairs)` → `json::Value::Object({...})`
- `Value::Option(None)` → `json::Value::Null`
- `Value::Option(Some(v))` → serialize the inner value
- `Value::Struct(name, fields)` → `json::Value::Object({field: val, ...})`
- `Value::Unit` → `json::Value::Null`

### `re.*`

**Recommended crate:** `regex`

Parser issue: `re.match` conflicts with the `match` keyword — fix Bug 10 first.

Functions to implement:
- `re.match(pattern: str, s: str) -> bool`
- `re.find(pattern: str, s: str) -> ?str`
- `re.findall(pattern: str, s: str) -> [str]`
- `re.replace(pattern: str, s: str, repl: str) -> str`
- `re.split(pattern: str, s: str) -> [str]`

### `http.*`

**Recommended crate:** `ureq` (synchronous, zero-dependency, good for scripting use case)

Functions to implement:
- `http.get(url: str) -> ?str` — GET, return body or None on error
- `http.post(url: str, body: str) -> ?str`
- `http.put(url: str, body: str) -> ?str`
- `http.del(url: str) -> ?str`
- `http.patch(url: str, body: str) -> ?str`

Note: These are synchronous. The `go.*` namespace handles async — `http.*` is blocking
by design (wrap in `go.spawn` for concurrency).

### `go.*` — Concurrency

**Recommended approach:** `std::thread` for `spawn`/`wait`, `std::sync::mpsc` for channels,
`std::thread::sleep` for `sleep`.

`Value::Task` needs to be added to the `Value` enum:
```rust
Task(Arc<Mutex<Option<Value>>>),  // future result
```

Functions to implement:
- `go.spawn(f: () => T) -> Task[T]` — spawn a thread, return handle
- `go.wait(task: Task[T]) -> T` — block until done
- `go.all(tasks: [Task[T]]) -> [T]` — wait for all
- `go.race(tasks: [Task[T]]) -> T` — return first completed
- `go.sleep(ms: int) -> void` — `std::thread::sleep(Duration::from_millis(ms))`

The tricky part is that `Value` is not `Send` (it contains `Fn(FnValue)` which has an
`Arc<dyn Fn>` but also `Env` with `Arc<Mutex<HashMap>>`). Making `Value: Send + Sync`
requires careful review — the `Env` globals `Arc<Mutex<...>>` are already `Send+Sync`,
but `FnBody::Native` uses `Arc<dyn Fn>` which requires `Send+Sync` bounds.

### `db.*`

**Recommended crate:** `rusqlite` for SQLite (pure Rust, no server), `sqlx` for multi-DB.

Start with SQLite only — URL `sqlite:///path/to/db.sqlite` or `sqlite::memory:`.

`Value::Connection` needs to be added to the `Value` enum:
```rust
Connection(Arc<Mutex<rusqlite::Connection>>),
```

Functions to implement:
- `db.connect(url: str) -> Connection` — open connection
- `db.query(conn, sql, args...) -> [{str: any}]` — return rows as list of maps
- `db.exec(conn, sql, args...) -> int` — return rows affected
- `db.close(conn) -> void` — close connection
- `db.tx(conn, f) -> T` — run function in transaction, rollback on error

### `cache.*`

Simplest implementation: in-memory `HashMap` with optional TTL using `std::time::Instant`.
Later: Redis via `redis` crate, using URL `redis://host:port`.

`Value::CacheHandle` is not needed — use a global static `Arc<Mutex<HashMap<String, (String, Option<Instant>)>>>`.

### `auth.*`

**Recommended crates:** `jsonwebtoken` for JWT, `bcrypt` for password hashing.

- `auth.jwt(payload, secret)` — sign a JWT, return token string
- `auth.verify(token, secret)` — verify and decode, return `?{str: any}`
- `auth.hash(password)` — bcrypt hash
- `auth.check(password, hash)` — bcrypt verify

### `queue.*`

Start with in-memory queues backed by `std::collections::VecDeque`. Use a global
`Arc<Mutex<HashMap<String, VecDeque<String>>>>`.

### `mail.*`, `store.*`, `ai.*`

These require external service credentials and are configuration-driven. Implement as
stubs that read config from environment variables:

- `mail.*` — use `SMTP_HOST`, `SMTP_USER`, `SMTP_PASS` env vars, `lettre` crate
- `store.*` — use `STORE_URL` env var, implement for local filesystem first
- `ai.*` — use `ANTHROPIC_API_KEY` env var, call Anthropic API via `http.post`

---

## MISSING — Codegen: function return type inference for non-integer types

**Symptom:**
```bash
ash build greet.ash   # fn greet(name)\n    "hello {name}"
# → LLVM IR error: '%r1' defined with type 'ptr' but expected 'i64'
```

**Root cause — `crates/ash-codegen/src/lib.rs`, `fn emit_fn()`:**

When a function has no return type annotation (e.g. `fn greet(name)`), the typechecker
sets `HirFn.ret = HirType::Unknown`. `hir_to_llvm(Unknown)` returns `LLVMType::I64`.
But the function body returns a string (an `i8*` pointer). The `ret i64 %r1` instruction
then fails because `%r1` is `ptr`.

The fix requires the codegen to infer the actual return type from the body's last
expression. When `cur_ret == I64` but the last expression has type `Ptr` (string), use
`Ptr` as the return type.

**Fix — `fn emit_fn()` in `crates/ash-codegen/src/lib.rs`:**

Track the actual type of the last expression:
```rust
let mut last: Option<(String, LLVMType)> = None;
for stmt in &f.body.stmts {
    if let Some((r, t)) = self.emit_stmt(stmt)? {
        last = Some((r, t));
    }
}

// Resolve return type: prefer the declared type, fall back to the actual type
let actual_ret = last.as_ref().map(|(_, t)| t.clone()).unwrap_or(self.cur_ret.clone());
let resolved_ret = if self.cur_ret == LLVMType::I64 && actual_ret == LLVMType::Ptr {
    LLVMType::Ptr  // Function returns string, not int
} else {
    self.cur_ret.clone()
};
```

Then use `resolved_ret` in the function signature `define` line and `ret` instruction.

Also fix the function signature emission — it must be emitted AFTER we know the resolved
return type, or functions need a two-pass approach (pre-scan body to determine return type,
then emit signature, then emit body).

---

## MISSING — Codegen: heap collections (lists, maps)

**Symptom:**
```
ash build program.ash   # any program using lists or maps
# → codegen: heap collections not yet in codegen
```

**Root cause — `crates/ash-codegen/src/lib.rs`:**

`HirExprKind::List`, `HirExprKind::Map`, `HirExprKind::Index` all return
`Err(CodegenError::new("heap collections not yet in codegen"))`.

**What's needed:**
A runtime representation for lists and maps in the compiled binary. Options:

**Option A (simple):** Fixed-size arrays allocated on the heap via `malloc`. Represent
`[int]` as a struct `{ i64 len; i64* data; }`. This requires:
- Adding `@ash_list_new`, `@ash_list_push`, `@ash_list_get` as declared external functions
- Implementing a small C runtime helper file (`ash_runtime.c`) that provides these
- Linking the runtime file when compiling: `clang program.ll ash_runtime.c -o program`

**Option B (deferred):** Only compile programs that don't use heap collections.
Add a check at the start of `compile()` that returns an error if the HIR contains
any list/map literals or index expressions.

Option A is the right long-term path. Start with just `[int]` — a list of integers.

---

## MISSING — Codegen: string interpolation and concatenation

**Symptom:**
```ash
fn greet(name)
    "hello {name}"    # interpolation with variable
# → compiles to garbage (returns pointer as integer)
```

**Root cause — `crates/ash-codegen/src/lib.rs`:**

String interpolation is handled at interpreter-runtime by `fn interpolate()`. The
codegen emits the raw template string `"hello {name}"` as a constant, without expanding
the `{name}` at compile time. There is no codegen path for string building.

`HirBinOp::StrConcat` exists in the HIR but `emit_binop()` returns
`Err(CodegenError::new("string concatenation not yet in compiled mode"))`.

**What's needed:**
- A `@ash_str_concat(i8*, i8*)` runtime function (simple `strcat` wrapper with malloc)
- A `@ash_str_from_int(i64)` function for interpolating integers
- A `@ash_str_from_float(double)` function for floats
- Lower string interpolation during codegen: split the template on `{...}`, emit each
  literal segment as a constant, emit each interpolated expression through the appropriate
  `@ash_str_from_X` call, then concatenate with `@ash_str_concat`

This is a significant chunk of work. Short-term: add an explicit error for string
interpolation in codegen pointing users to `ash run` for string-heavy programs.

---

## MISSING — Typechecker: not used during `ash run`

The interpreter walks the AST directly and does no type checking. Type annotations are
parsed but ignored at runtime. The typechecker (`ash-typeck`) is only invoked by
`ash build` (codegen path) and `ash check`.

**Current behavior:**
```ash
fn add(a: int, b: int): int
    a + b
add("hello", 2)   # no error — runs, returns "hello2" (string concat)
```

**What needs to change:**
For `ash run`, optionally run `ash-typeck` before interpretation and surface type errors.
This would be a flag: `ash run --strict file.ash` or just always-on.

The bigger issue: `ash check` calls the typechecker but only reports counts, not
actual type errors. The typechecker does infer types but error reporting is minimal.

---

## MISSING — Codegen: `println` format string for strings

**Symptom:**
```ash
fn name(): str
    "world"
println(name())   # prints garbage number, not "world"
```

**Root cause — `crates/ash-codegen/src/lib.rs`, `fn emit_named_call()`, `"println"` arm:**

```rust
let f = match self.infer_arg_llvm_ty(args.first()) {
    LLVMType::I64 => "%ld", LLVMType::Double => "%f",
    LLVMType::I1 => "%d",  LLVMType::Ptr => "%s", _ => "%d",
};
```

When a function call's return type is inferred as `I64` (because the function has no
annotation and the codegen defaults unknown to I64), `%ld` is used, but the actual
value is a pointer. This is the same root issue as Bug in emit_fn above — return type
inference needs to be fixed first.

**Quick fix:** When calling `println` with a `call i64 @fn()` result, check the
actual declared return type in `fn_sigs`. If it's `Ptr`, use `%s`.

---

## MISSING — `ash check` type error reporting

**Current behavior:**
```
ash check file.ash   # always says "OK (N fns, M types, K stmts)" even for type errors
```

**What should happen:**
Real type errors should be reported. The typechecker does partially infer types, but its
error reporting is minimal — it returns `Err(String)` on failure but the errors are
caught by `ash check` which only checks if it panics.

Implement proper diagnostics: type mismatch errors with line numbers, undefined variable
errors (already works at runtime, needs to work at typecheck time too).

---

## MINOR — `list.join` not implemented

**Symptom:**
```
["a", "b", "c"].join(", ")   → runtime: no method 'join' on [a, b, c]
```

**Fix — `crates/ash-interp/src/lib.rs`, `fn call_method()`:**

```rust
(Value::List(items), "join") => {
    let sep = match args.first() {
        Some(Value::Str(s)) => s.clone(),
        _ => return Err(InterpError::runtime("join() requires a string separator")),
    };
    Ok(Value::Str(items.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(&sep)))
}
```

---

## MINOR — `list.set` not implemented (immutable update)

**Symptom:**
```
[1, 2, 3].set(1, 99)   → runtime: no method 'set' on [1, 2, 3]
```

Note: In-place mutation `items[1] = 99` works. `.set()` is for immutable update that
returns a new list (functional style).

**Fix — `crates/ash-interp/src/lib.rs`, `fn call_method()`:**

```rust
(Value::List(items), "set") => {
    if args.len() < 2 { return Err(InterpError::runtime("set() requires index and value")); }
    let idx = match &args[0] { Value::Int(n) => *n as usize, _ => return Err(InterpError::runtime("set() index must be int")) };
    let mut new_items = items.clone();
    if idx >= new_items.len() { return Err(InterpError::runtime(format!("set() index {idx} out of bounds"))); }
    new_items[idx] = args[1].clone();
    Ok(Value::List(new_items))
}
```

---

## MINOR — `map.set` not implemented (immutable update)

Similar to `list.set` — maps have `.get`, `.has`, `.keys`, `.vals`, `.len` but not `.set`.
In-place map mutation is not currently possible at all (no `m["key"] = val` syntax for maps).

**Fix — `crates/ash-interp/src/lib.rs`, `fn call_method()`:**

```rust
(Value::Map(pairs), "set") => {
    if args.len() < 2 { return Err(InterpError::runtime("map.set() requires key and value")); }
    let mut new_pairs = pairs.clone();
    let key = args[0].clone();
    let val = args[1].clone();
    if let Some(pos) = new_pairs.iter().position(|(k, _)| k == &key) {
        new_pairs[pos] = (key, val);
    } else {
        new_pairs.push((key, val));
    }
    Ok(Value::Map(new_pairs))
}
```

Also need `map.del(key)`:
```rust
(Value::Map(pairs), "del") => {
    let key = args.first().ok_or_else(|| InterpError::runtime("del() requires key"))?;
    Ok(Value::Map(pairs.into_iter().filter(|(k, _)| k != key).collect()))
}
```

---

## MINOR — `fmt()` with `{}` is broken (see Bug 4 above)

The `fmt()` builtin already does `result.replacen("{}", &arg.to_string(), 1)` correctly —
the bug is that `{}` gets consumed by `interpolate()` before `fmt()` sees it.
Fix Bug 4 first, then `fmt()` will work.

---

## MINOR — `env.set` not implemented

`env.get` and `env.require` are registered. `env.set` is not.

**Fix — `crates/ash-interp/src/lib.rs`, `register_stdlib()`:**

```rust
self.define("env.set", Value::Fn(FnValue {
    name: Some("env.set".into()), params: vec![],
    body: FnBody::Native(Arc::new(|args| {
        if args.len() < 2 { return Err(InterpError::runtime("env.set requires key and value")); }
        match (&args[0], &args[1]) {
            (Value::Str(k), Value::Str(v)) => {
                std::env::set_var(k, v);
                Ok(Value::Unit)
            }
            _ => Err(InterpError::runtime("env.set requires string key and value")),
        }
    })),
    closure: Env::default(),
}));
```

---

## MINOR — `file.append` not implemented

`file.read`, `file.write`, `file.exists`, `file.ls`, `file.rm`, `file.mkdir` are all
registered. `file.append` is defined in ash-stdlib but not registered in the interpreter.

**Fix — `crates/ash-interp/src/lib.rs`, `register_stdlib()`:**

```rust
self.define("file.append", Value::Fn(FnValue {
    name: Some("file.append".into()), params: vec![],
    body: FnBody::Native(Arc::new(|args| {
        use std::io::Write;
        if args.len() < 2 { return Err(InterpError::runtime("file.append requires path and data")); }
        match (&args[0], &args[1]) {
            (Value::Str(path), Value::Str(data)) => {
                let mut f = std::fs::OpenOptions::new()
                    .append(true).create(true).open(path)
                    .map_err(|e| InterpError::runtime(e.to_string()))?;
                f.write_all(data.as_bytes()).map_err(|e| InterpError::runtime(e.to_string()))?;
                Ok(Value::Unit)
            }
            _ => Err(InterpError::runtime("file.append requires string path and data")),
        }
    })),
    closure: Env::default(),
}));
```

---

## MINOR — `math.*` missing functions

`math.clamp` is defined in the stdlib docs but not registered in the interpreter
(only the top-level `clamp(x, lo, hi)` is registered).

**Fix:** Add `math.clamp` to `register_stdlib()`:
```rust
self.define("math.clamp", Value::Fn(FnValue {
    name: Some("math.clamp".into()), params: vec![],
    body: FnBody::Native(Arc::new(|args| {
        if args.len() < 3 { return Err(InterpError::runtime("math.clamp requires 3 args")); }
        match (&args[0], &args[1], &args[2]) {
            (Value::Float(x), Value::Float(lo), Value::Float(hi)) => Ok(Value::Float(x.clamp(*lo, *hi))),
            (Value::Int(x), Value::Int(lo), Value::Int(hi)) => Ok(Value::Int((*x).clamp(*lo, *hi))),
            _ => Err(InterpError::runtime("math.clamp requires numbers"))
        }
    })),
    closure: Env::default(),
}));
```

---

## TOOLING — Module system (multi-file programs)

Currently every `ash run` is a single self-contained file. There is no way to split
a program across multiple files.

**What needs to be built:**

1. A file resolver: given a module reference like `db.connect` in a source file, look for
   `db/connect.ash` or `db.ash` relative to the source file's directory.

2. A program loader: before running `main.ash`, recursively discover and parse all `.ash`
   files in the project directory. Functions/types defined in `models/user.ash` become
   accessible as `models.user.User` (or just `User` within the models module).

3. A dependency graph: detect and reject circular imports.

This is a significant architectural addition. The simplest first step: allow
`use "path/to/file.ash"` as an explicit import statement that inlines the file's
definitions.

---

## TOOLING — Package manager

No `ash.toml` format exists. No external package installation mechanism exists.

Start small: define an `ash.toml` format:
```toml
[package]
name = "my-app"
version = "0.1.0"

[dependencies]
ash-http = "0.1"
```

External packages would be plain `.ash` files (or compiled `.ashlib` bundles) fetched
from a registry or git URL.

---

## TOOLING — LSP server (syntax highlighting & editor integration)

No language server exists for Ash. Without one, editors cannot provide syntax
highlighting, go-to-definition, hover docs, or diagnostics.

**What needs to be built:** A crate `ash-lsp` that implements the
[Language Server Protocol](https://microsoft.github.io/language-server-protocol/)
so any LSP-capable editor (VS Code, Neovim, Helix, …) works out of the box.

**Recommended crate:** `tower-lsp` (async, well-maintained, handles JSON-RPC
transport automatically).

**Minimum viable feature set:**

1. **Syntax highlighting via semantic tokens** — lex the file, emit token types
   (`keyword`, `string`, `number`, `function`, `variable`, `type`, `operator`,
   `comment`) as `textDocument/semanticTokens/full` responses.
   The lexer (`ash-lexer`) is already production-ready — map each `Token`
   variant to a semantic token type.

2. **Parse diagnostics** — run the parser on every `textDocument/didChange`
   notification; surface `ParseError` as LSP `Diagnostic` objects with accurate
   line/column positions. The `Span` type already carries this information.

3. **Hover** — on `textDocument/hover`, look up the identifier under the cursor
   in the stdlib descriptor table (`ash-stdlib`) and return its signature + doc
   comment as a `MarkupContent` response.

4. **Go-to-definition** — on `textDocument/definition`, resolve identifiers to
   their definition span using a simple name→span map built during parsing.

**CLI integration:** Add `ash lsp` as a new subcommand in `ash-cli/src/main.rs`
that starts the language server in stdio mode (standard for LSP).

**VS Code extension stub:** A minimal `ash-vscode/` directory with a
`package.json` that activates the extension for `*.ash` files and launches
`ash lsp` as the language server. No grammar file needed once semantic tokens
are working.

**TextMate grammar (fallback):** For editors that don't support semantic tokens
(GitHub, GitLab web views), a `syntaxes/ash.tmLanguage.json` grammar file can
provide basic highlighting using regex rules derived from the keyword list.

---

## TOOLING — Test runner

No `ash test` command. Tests would be written as:

```ash
fn test_add()
    assert(add(2, 3) == 5, "2 + 3 should be 5")

fn test_empty_list()
    assert(filter([], x => true).len() == 0, "filter of empty is empty")
```

Add `assert(cond: bool, msg: str)` as a core builtin, and `ash test` as a CLI command
that finds all functions named `test_*`, runs them, and reports pass/fail counts.

---

## Implementation status

**All items completed ✓**

1. ✓ Bug 1 — `none`, `Some`, `Ok`, `Err` registered as global names
2. ✓ Bug 4 — `{}` in string interpolation left as literal `{}`
3. ✓ Minor — `list.join`, `list.set` implemented
4. ✓ Minor — `file.append` implemented
5. ✓ Minor — `env.set` implemented
6. ✓ Bug 5 — Range `0..n` parsing works
7. ✓ Bug 3 — Map literal `{"k": v}` parsing works
8. ✓ Bug 9 — Tuple pattern in match works
9. ✓ Bug 8 — Struct pattern in match works
10. ✓ Bug 10 — `re.match` keyword conflict resolved
11. ✓ Bug 2 — `!` error propagation fixed end-to-end
12. ✓ `json.*` — parse, str, pretty (serde_json)
13. ✓ `re.*` — match, find, findall, replace, split (regex crate)
14. ✓ Bug 7 — Type annotation on bare assignment (`x: int = 5`)
15. ✓ Bug 6 — Multi-line pipeline chains
16. ✓ `http.*` — get/post/put/del/patch (ureq)
17. ✓ Codegen: return type inference for string/bool functions
18. ✓ Codegen: heap lists via ash_runtime.c (ash_list_*)
19. ✓ Codegen: string interpolation lowered to StrConcat chains
20. ✓ `go.*` — sleep, wait, all, spawn (stub)
21. ✓ `db.*` — SQLite via rusqlite (connect/exec/query/close)
22. ✓ Module system — `use "path.ash"` inline imports
23. ✓ `ash test` command
24. ✓ LSP server (`ash lsp`, semantic tokens, diagnostics, hover, go-to-def)
25. ✓ `cache.*` (in-memory TTL), `queue.*` (named FIFO queues), `auth.*` (stubs)
26. ✓ `ai.*` (Anthropic API), `mail.*` (stub), `store.*` (stub)

**Remaining / nice-to-have:**
- `go.spawn` proper interpreter integration (currently stub — requires interpreter dispatch)
- `auth.*` real implementation (bcrypt + jsonwebtoken crates)
- `mail.*` real implementation (lettre crate)
- `store.*` real implementation
- Codegen: map/tuple literals (currently error with suggestion to use `ash run`)
- Package manager (`ash.toml` + registry)
- `ash check --strict` mode (run typechecker before interpreter)
