# Ash

**By AI, for AI.**

Ash is a programming language designed from the ground up to be written by AI systems and read by humans. Every decision — syntax, type system, standard library, memory model — is optimized for one thing: generating correct, expressive code in as few tokens as possible.

The result is a language that is shorter than Python, faster than Go when compiled, and ships with first-class primitives for the things modern AI-generated applications actually need: HTTP, databases, queues, authentication, blob storage, JSON, regex, and AI inference — all available without a single import statement.

---

## The idea

When a language model writes code, it pays a cost per token. That cost compounds across millions of calls. A language designed for human readability trades token efficiency for familiarity — verbose keywords, mandatory type annotations, boilerplate constructors, import ceremonies.

Ash makes the opposite trade. It is optimized for the author being a machine.

- **No imports** — the entire standard library is in scope by default
- **Full type inference** — types are inferred everywhere; annotations are allowed, never required
- **Expression-oriented** — the last expression in a block is its value; no `return` needed except for early exit
- **Structural typing** — pass anything with the right shape; no named interfaces required
- **Indentation blocks** — zero tokens spent on braces or `end` keywords
- **Lambdas without syntax** — `x => x + 1` is a complete lambda
- **Pipelines** — `data |> filter(x => x > 0) |> map(x => x * 2)` chains left to right
- **Expression interpolation** — `"hello {name}, you have {count * 2} items"` supports full expressions
- **Option and Result built in** — `?T`, `?.`, `??`, and `!` propagation with no imports

When an AI writes Ash it uses 4.6% fewer tokens than Python and 65% fewer than Java for equivalent logic. When that code is compiled, it runs faster than Go.

---

## Quick start

```bash
cargo build --release
./target/release/ash run examples/hello.ash
./target/release/ash repl
```

---

## A taste of the language

```ash
# Types — structural, inferred
type User
    id:    int
    name:  str
    admin: bool

# Functions — last expression is the return value, no return keyword needed
fn greet(user)
    if user.admin
        "Hello admin {user.name}"
    else
        "Hello {user.name}"

# Pipelines + lambdas
fn active_names(users)
    users
        |> filter(u => u.admin == false)
        |> map(u => u.name)
        |> filter(n => n.len() > 3)

# Union types + pattern matching
type Shape = Circle(float) | Rect(float float)

fn area(s)
    match s
        Circle(r)   => math.pi * r * r
        Rect(w h)   => w * h

# Null safety
user = find_user(42)            # returns ?User
name = user?.name ?? "unknown"  # safe navigation + fallback

# Error propagation
fn load_config(path)
    raw  = file.read(path)!    # propagates Err up the call stack
    json.parse(raw) ?? {}

# No imports ever — stdlib always in scope
println(math.sqrt(144.0))
file.write("/tmp/out.txt", "hello")
home = env.get("HOME") ?? "/root"
```

---

## Two modes, one language

**`ash run`** — tree-walking interpreter. Zero build step, starts in milliseconds, rich error messages. Ideal for development, scripting, and prototyping.

**`ash build`** — compiles to LLVM IR, then to a native binary via clang. No runtime overhead, no GC, no startup cost. Runs faster than Go on compute benchmarks.

```bash
ash run   hello.ash              # interpret
ash build hello.ash -o hello     # compile to native binary
ash check hello.ash              # type-check without running
ash fmt   hello.ash              # auto-format in place
ash docs  math                   # browse stdlib docs
ash repl                         # interactive REPL
```

---

## Benchmark results

**Compute benchmark:** `fib(25)` + `count_primes(1000)` + `collatz(27)` + `sum_range(10000)`.

| Mode | ms/run | vs Python |
|------|--------|-----------|
| **Ash compiled** | **6ms** | **11.5x faster** |
| Go native | 8ms | 8.6x faster |
| Python | 69ms | baseline |
| Java JVM | 150ms | 2.2x slower |
| Ash interpreted | 1049ms | 15x slower |

**Token count benchmark:** identical todo CRUD backend.

| Language | Tokens | vs Python |
|----------|--------|-----------|
| **Ash** | **334** | **-4.6%** |
| Python | 350 | baseline |
| Go | 487 | +39% |
| Java | 578 | +65% |

Run `./benchmark/run_benchmark.sh` to reproduce timing.
Run `python3 benchmark/token_count.py` to reproduce token counts.

---

## Standard library

Everything is in scope. Nothing to import.

| Namespace | What it does |
|-----------|-------------|
| core | `print`, `println`, `fmt`, `abs`, `min`, `max`, `clamp`, `filter`, `map`, `reduce`, `zip`, `flat`, `any`, `all`, `int`, `float`, `str`, `bool` |
| `math.*` | `sqrt`, `pow`, `floor`, `ceil`, `round`, `sin`, `cos`, `tan`, `log`, `pi`, `e` |
| `file.*` | `read`, `write`, `append`, `exists`, `ls`, `rm`, `mkdir` |
| `http.*` | `get`, `post`, `put`, `del`, `patch`, `fetch` |
| `json.*` | `parse`, `str`, `pretty` |
| `re.*` | `match`, `find`, `findall`, `replace`, `split` |
| `env.*` | `get`, `require`, `all`, `set` |
| `go.*` | `spawn`, `wait`, `all`, `race`, `sleep`, `chan` |
| `db.*` | `connect`, `query`, `exec`, `tx` |
| `cache.*` | `get`, `set`, `setex`, `del`, `flush` |
| `queue.*` | `push`, `pop`, `sub`, `len` |
| `auth.*` | `jwt`, `verify`, `hash`, `check` |
| `mail.*` | `send`, `html` |
| `store.*` | `put`, `get`, `del`, `url`, `list` |
| `ai.*` | `complete`, `chat`, `embed`, `similarity`, `classify`, `moderate` |

---

## Architecture

```
ash/
├── crates/
│   ├── ash-lexer/      Tokenizer — indent/dedent synthesis
│   ├── ash-parser/     Recursive-descent parser -> AST
│   ├── ash-hir/        High-level IR — fully typed, desugared
│   ├── ash-typeck/     Type inference — AST -> HIR
│   ├── ash-interp/     Tree-walking interpreter (ash run)
│   ├── ash-codegen/    LLVM IR emitter consuming typed HIR (ash build)
│   └── ash-stdlib/     Stdlib definitions and runtime stubs
├── ash-cli/            The ash binary
├── examples/           Example programs
└── benchmark/          Speed and token benchmarks
```

---

## Tests

```bash
cargo test --workspace   # 188 tests, 0 failures
```

---

## Language reference

See [`LANGUAGE.md`](LANGUAGE.md) for the complete language specification.

---

## License

MIT
