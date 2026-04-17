# Ash Language Reference

Complete specification of the Ash programming language.

---

## Table of contents

1. [Syntax fundamentals](#1-syntax-fundamentals)
2. [Variables and bindings](#2-variables-and-bindings)
3. [Types](#3-types)
4. [Expressions](#4-expressions)
5. [Functions](#5-functions)
6. [Control flow](#6-control-flow)
7. [Pattern matching](#7-pattern-matching)
8. [Collections](#8-collections)
9. [Strings](#9-strings)
10. [Null safety and error handling](#10-null-safety-and-error-handling)
11. [User-defined types](#11-user-defined-types)
12. [Generics](#12-generics)
13. [Memory model](#13-memory-model)
14. [Concurrency](#14-concurrency)
15. [Module system](#15-module-system)
16. [Standard library](#16-standard-library)
17. [The two backends](#17-the-two-backends)

---

## 1. Syntax fundamentals

### Indentation

Blocks are delimited by indentation, not braces. The standard indent is 4 spaces. Tabs are accepted and treated as 4 spaces.

```ash
fn greet(name)
    msg = "hello {name}"
    println(msg)
```

Parentheses, brackets, and braces suppress newlines. Multi-line expressions inside them are fine:

```ash
result = filter(
    [1, 2, 3, 4, 5],
    x => x > 2
)
```

### Statement separation

Statements are separated by newlines. A semicolon `;` allows multiple statements on one line:

```ash
x = 1; y = 2; z = x + y
```

### Comments

```ash
# This is a comment — everything after # on a line is ignored
x = 5  # inline comment
```

---

## 2. Variables and bindings

### Immutable by default

```ash
x = 42          # immutable binding
name = "ash"    # string binding
```

Reassigning an immutable binding is a compile-time error.

### Mutable bindings

```ash
mut counter = 0
counter = counter + 1  # ok
```

### Explicit let

The `let` keyword is optional but allowed for clarity:

```ash
let x = 42
let mut y = 0
```

### Type annotations

Annotations are optional. Use `:` after the name:

```ash
x: int = 42
name: str = "ash"
let mut total: float = 0.0
```

### Assignment

Reassignment uses `=`. Targets can be variables, list indices, or struct fields:

```ash
x = 99
items[0] = "first"
user.name = "Lorenzo"
```

---

## 3. Types

### Primitive types

| Type | Description | Literals |
|------|-------------|---------|
| `int` | 64-bit signed integer | `42`, `-7`, `0` |
| `float` | 64-bit double | `3.14`, `-0.5`, `1.0` |
| `bool` | Boolean | `true`, `false` |
| `str` | UTF-8 string | `"hello"`, `'world'` |
| `void` | No value (unit) | — |

### Compound types

```ash
[int]           # list of int
{str: int}      # map from str to int
(int str)       # tuple
?int            # option — int or nothing
Result[int str] # result — int on success, str on error
```

### Type inference

Types are inferred from usage. You never need to write a type unless you want to:

```ash
x = 5          # inferred int
y = 3.14       # inferred float
items = [1,2]  # inferred [int]
```

### Structural typing

Function parameters are structurally typed — any value with the required fields works:

```ash
fn area(shape)
    shape.width * shape.height

# Both of these work — no explicit interface needed
area(Rectangle { width: 10, height: 5 })
area(Photo { width: 1920, height: 1080, url: "..." })
```

### Nominal typing

User-defined `type` declarations are nominal — two types with the same fields are distinct:

```ash
type Meters
    value: float

type Kilograms
    value: float

# Meters and Kilograms are different types even though their structure is identical
```

---

## 4. Expressions

### Arithmetic

```ash
x + y   # addition (int or float; also string concatenation for str + str)
x - y   # subtraction
x * y   # multiplication
x / y   # division (integer division for int / int)
x % y   # modulo
-x      # negation
```

### Comparison

```ash
x == y   # equal
x != y   # not equal
x < y    # less than
x > y    # greater than
x <= y   # less than or equal
x >= y   # greater than or equal
```

### Logic

```ash
a && b   # and (short-circuits)
a || b   # or (short-circuits)
!a       # not
```

### Pipelines

The `|>` operator passes the left side as the first argument to the right side:

```ash
5 |> double             # same as double(5)
items |> filter(x => x > 0)   # same as filter(items, x => x > 0)
items |> filter(x => x > 0) |> map(x => x * 2)  # chains
```

### Lambdas

```ash
x => x * 2              # single parameter
(x y) => x + y          # multiple parameters
(x y) => x + y + z      # closes over z from surrounding scope
```

Lambdas are first-class values and can be stored and passed around:

```ash
double = x => x * 2
apply = (f x) => f(x)
apply(double, 5)   # 10
```

### Blocks

A block is a sequence of statements in braces — the last expression is the block's value:

```ash
result = if x > 0
    "positive"
else
    "non-positive"
```

---

## 5. Functions

### Definition

```ash
fn add(a b)
    a + b

fn greet(name: str): str
    "hello {name}"
```

- Parameters are separated by spaces (not commas)
- Return type annotation after `):`  is optional
- Last expression is the implicit return value
- `return` is for early exit only

### Calling

```ash
add(3, 4)      # with commas
add(3 4)       # without commas — both work
```

### Generics

Single uppercase letter is an implicit type variable:

```ash
fn first(items: [T]): ?T
    items[0]
```

Multiple type variables use explicit brackets:

```ash
fn zip[T U](a: [T], b: [U]): [(T U)]
    # ...
```

### Borrowing

The `&` prefix on a parameter means the caller's value is borrowed, not moved:

```ash
fn print_user(&u: User)
    println(u.name)

fn update_name(&mut u: User, name: str)
    u.name = name
```

### Recursion

Functions can call themselves directly:

```ash
fn factorial(n: int): int
    if n <= 1
        1
    else
        n * factorial(n - 1)
```

---

## 6. Control flow

### If / else

```ash
if x > 0
    "positive"
else if x < 0
    "negative"
else
    "zero"
```

`if` is an expression — it returns the value of the executed branch:

```ash
label = if score >= 90 then "A" else "B"
```

### While

```ash
mut i = 0
while i < 10
    println(i)
    i = i + 1
```

### For

Iterates over lists, ranges, strings, or any iterable:

```ash
for x in [1, 2, 3]
    println(x)

for i in 0..10
    println(i)

for ch in "hello"
    println(ch)
```

### Panic

Terminates the program with a message. Use only for unrecoverable errors:

```ash
panic "unreachable state reached"
```

---

## 7. Pattern matching

```ash
match value
    0     => "zero"
    1     => "one"
    n     => "other: {n}"      # binds the matched value to n

match shape
    Circle(r)   => math.pi * r * r
    Rect(w h)   => w * h
    _           => 0            # wildcard

match result
    Ok(v)  => v * 2
    Err(e) => panic e
```

Match is exhaustive — the compiler warns if a case is missing. Use `_` as a wildcard catch-all.

### Literal patterns

```ash
match status
    200 => "ok"
    404 => "not found"
    _   => "other"
```

### Tuple patterns

```ash
match (x, y)
    (0, 0) => "origin"
    (x, 0) => "x-axis at {x}"
    (0, y) => "y-axis at {y}"
    (x, y) => "point ({x}, {y})"
```

---

## 8. Collections

### Lists

```ash
items = [1, 2, 3, 4, 5]
items.len()          # 5
items[0]             # some(1) — indexing returns ?T
items[0] ?? 0        # 1 — with fallback
items.push(6)        # returns new list (immutable by default)
items.first()        # some(1)
items.last()         # some(5)
items.reverse()      # [5, 4, 3, 2, 1]
items.sort()         # [1, 2, 3, 4, 5]
items.sort(x => -x)  # [5, 4, 3, 2, 1] — sort by key
items.contains(3)    # true
items.filter(x => x > 2)       # [3, 4, 5]
items.map(x => x * 2)          # [2, 4, 6, 8, 10]
items.reduce((acc x) => acc + x, 0)  # 15
```

Index mutation works on `mut` lists:

```ash
mut items = [1, 2, 3]
items[1] = 99     # [1, 99, 3]
```

### Maps

```ash
scores = {"alice": 95, "bob": 82}
scores.get("alice")        # some(95)
scores.get("charlie")      # none
scores.has("bob")          # true
scores.keys()              # ["alice", "bob"]
scores.vals()              # [95, 82]
scores.set("charlie", 78)  # returns new map
scores.len()               # 2
```

### Tuples

```ash
pair = (1, "hello")
triple = (3.14, true, "pi")
```

---

## 9. Strings

### Literals

```ash
"double quoted"
'single quoted'
"with \n newline \t tab"
```

### Interpolation

Any expression can go inside `{}`:

```ash
name = "Lorenzo"
n    = 42
"Hello {name}, answer is {n}"
"double is {n * 2}"
"pi is {math.pi}"
"len is {items.len()}"
```

### Methods

```ash
s = "  Hello World  "
s.len()              # 15
s.upper()            # "  HELLO WORLD  "
s.lower()            # "  hello world  "
s.trim()             # "Hello World"
s.split(" ")         # ["", "", "Hello", "World", "", ""]
s.contains("World")  # true
s.starts("  He")     # true
s.ends("  ")         # true
s.replace("World", "Ash")  # "  Hello Ash  "
s.find("World")      # some(8)
```

### fmt function

```ash
fmt("Hello {}, you have {} messages", name, count)
# same as "{name}, you have {count} messages" interpolation
```

---

## 10. Null safety and error handling

### Option type `?T`

A value that may be absent. Ash has no null — only `?T`.

```ash
find_user(id)       # returns ?User

# Safe navigation — propagates none
user?.name          # type: ?str
user?.address?.city  # type: ?str

# Null coalescing — provide a default
user?.name ?? "anon"   # str

# Explicit match
match find_user(id)
    Some(u) => u.name
    None    => "not found"
```

### Result type `Result[T E]`

A value that is either a success or a typed error:

```ash
type Result = Ok(int) | Err(str)  # user-defined
# or the built-in Result[T E] generic
```

### Error propagation `!`

The `!` postfix operator short-circuits out of the current function with the `Err` value if the expression is an error:

```ash
fn read_config(path: str): Result[Config str]
    raw    = file.read(path)!     # returns Err if file missing
    parsed = json.parse(raw)!     # returns Err if invalid JSON
    Ok(parsed)
```

### Both together

```ash
fn process(path: str): ?int
    content = file.read(path)?    # none if file missing
    n = int(content.trim())?      # none if not a valid int
    Some(n * 2)
```

---

## 11. User-defined types

### Struct types

```ash
type Point
    x: float
    y: float

type User
    id:    int
    name:  str
    email: str
    admin: bool
```

### Struct literals

```ash
p = Point { x: 1.0, y: 2.0 }
u = User { id: 1, name: "Lorenzo", email: "l@example.com", admin: false }
```

### Field access and mutation

```ash
println(p.x)         # field read
p.x = 3.0            # field mutation (p must be mut)
u.name = "Alex"
```

### Union types

```ash
type Shape = Circle(float) | Rect(float float) | Triangle(float float float)

type Color = Red | Green | Blue  # unit variants (no payload)

type Result[T E] = Ok(T) | Err(E)  # generic union
```

### Constructing union variants

Variants are constructor functions — call them like functions:

```ash
c = Circle(5.0)
r = Rect(3.0, 4.0)
color = Red()
ok = Ok(42)
err = Err("something went wrong")
```

---

## 12. Generics

### Implicit single type variable

A single uppercase letter in a function signature is automatically a generic type variable:

```ash
fn identity(x: T): T
    x

fn first(items: [T]): ?T
    items[0]

fn map_fn(items: [T], f: T => U): [U]
    items.map(f)
```

### Explicit multiple type variables

When you need more than one, declare them with `[T U ...]`:

```ash
fn zip[T U](a: [T], b: [U]): [(T U)]
    # ...

fn fold[T A](items: [T], f: (A T) => A, init: A): A
    items.reduce(f, init)
```

### Generic type constraints (coming soon)

```ash
fn max[T: Comparable](a: T, b: T): T
    if a > b then a else b
```

---

## 13. Memory model

### Ownership

Every value has exactly one owner. Assigning a non-primitive to a new name **moves** it — the old name is no longer valid:

```ash
items = [1, 2, 3]
other = items      # items is moved — no longer valid
```

### Borrowing

Pass a reference with `&` to let a function read without taking ownership:

```ash
fn print_list(&items: [int])
    for x in items
        println(x)

print_list(&my_list)   # my_list still valid after the call
```

### Mutable borrowing

A mutable borrow allows the called function to mutate the caller's value:

```ash
fn push_item(&mut items: [int], x: int)
    items = items.push(x)
```

### Primitives always copy

`int`, `float`, `bool` are always copied on assignment — no move semantics for primitives.

### Move into closures

Closures capture by borrow by default. Use `move` to force ownership transfer:

```ash
items = [1, 2, 3]
f = move => items.len()   # items is moved into the closure
```

### Lexical lifetimes

Values live until the end of their lexical scope. The compiler rejects programs with use-after-move or use-after-scope errors. No lifetime annotations are ever required.

---

## 14. Concurrency

Ash uses a goroutine-style model — lightweight tasks with channels. The `await` keyword blocks for a result but no function needs to be marked `async`.

### Spawning tasks

```ash
task = go.spawn(() => fetch_data("https://api.example.com"))
result = await task
```

### Waiting for multiple tasks

```ash
tasks = [go.spawn(fn1), go.spawn(fn2), go.spawn(fn3)]
results = await go.all(tasks)   # waits for all
first   = await go.race(tasks)  # first to finish
```

### Channels

```ash
ch = go.chan()
go.spawn(() => ch.send(compute_value()))
value = ch.recv() ?? 0
```

### Sleep

```ash
go.sleep(500)   # sleep 500 milliseconds
```

---

## 15. Module system

### Inline imports with `use`

Include another `.ash` file's definitions into the current file:

```ash
use "models/user.ash"
use "helpers/format.ash"

u = User { id: 1, name: "Alice" }
```

The `use` statement inlines the file's definitions at the point of inclusion. Circular
imports are detected and skipped automatically.

### Files and folders (planned)

The long-term module model is folder-based:

- A folder is a module — its name is the module name
- Files inside the folder are submodules
- Qualify with the module path:

```
src/
  main.ash          # top-level code
  models/
    user.ash        # models.User lives here
```

> **Status:** Folder-based auto-discovery is not yet implemented. Use explicit
> `use "path.ash"` statements in the meantime.

### External packages

> **Status:** No package manager yet. External packages must be vendored manually
> and loaded with `use`.

### Circular dependencies

Circular `use` imports are detected and skipped (the second occurrence is a no-op).
Compile-time errors for circular folder-based imports are planned.

---

## 16. Standard library

The entire standard library is in scope without any import. Namespaces avoid collisions.

### Core (no prefix)

```ash
print(x)               # write to stdout, no newline
println(x)             # write to stdout with newline
read()                 # read one line from stdin
fmt("hello {}" name)   # format string with placeholders
int(x)                 # convert to int
float(x)               # convert to float
str(x)                 # convert to string
bool(x)                # convert to bool
abs(x)                 # absolute value
min(a b)               # minimum of two values
max(a b)               # maximum of two values
clamp(x lo hi)         # clamp x between lo and hi
filter(list f)         # keep elements where f returns true
map(list f)            # transform each element
reduce(list f init)    # fold to single value
zip(a b)               # pair elements from two lists
flat(list)             # flatten one level of nesting
any(list f)            # true if any element matches
all(list f)            # true if all elements match
panic(msg)             # terminate with message
```

### math.*

```ash
math.floor(x)    math.ceil(x)    math.round(x)
math.sqrt(x)     math.pow(x e)   math.log(x)
math.log2(x)     math.log10(x)   math.abs(x)
math.sin(x)      math.cos(x)     math.tan(x)
math.pi                           math.e
```

### file.*

```ash
file.read(path)            # ?str — None if file missing
file.write(path data)      # void
file.append(path data)     # void
file.exists(path)          # bool
file.ls(dir)               # [str]
file.rm(path)              # void
file.mkdir(path)           # void
```

### http.*

```ash
http.get(url)              # ?str
http.post(url body)        # ?str
http.put(url body)         # ?str
http.del(url)              # ?str
http.patch(url body)       # ?str
http.fetch(url opts)       # Response — full control
```

### json.*

```ash
json.parse(s)      # ?any — None if invalid
json.str(x)        # str — serialize to JSON
json.pretty(x)     # str — pretty-printed JSON
```

### re.*

```ash
re.match(pattern s)          # bool
re.find(pattern s)           # ?str
re.findall(pattern s)        # [str]
re.replace(pattern s repl)   # str
re.split(pattern s)          # [str]
```

### env.*

```ash
env.get(key)       # ?str
env.require(key)   # str — panics if missing
env.all()          # {str: str}
env.set(key val)   # void
```

### go.* (concurrency)

> **`go.sleep`** is fully implemented.
> **`go.spawn`** is a stub — it returns an error. `go.wait`, `go.all`, `go.race`,
> and `go.chan` are registered stubs pending the `go.spawn` implementation.

```ash
go.spawn(f)        # Task[T] — stub
go.wait(task)      # T       — stub
go.all(tasks)      # [T]     — stub
go.race(tasks)     # T       — stub
go.sleep(ms)       # void    — implemented
go.chan()          # Chan[T] — stub
```

### db.*

Backed by **SQLite** via `rusqlite`. Connection URLs:
- `sqlite:///path/to/db.sqlite` — file-based database
- `sqlite::memory:` — in-memory database (lost on close)

```ash
db.connect(url)           # Connection
db.query(conn sql args)   # [{str: any}] — rows as list of maps
db.exec(conn sql args)    # int (rows affected)
db.tx(conn f)             # T (auto-rollback on error)
db.close(conn)            # void
```

Positional parameters use `?` placeholders:
```ash
conn = db.connect("sqlite:///app.db")
rows = db.query(conn, "SELECT * FROM users WHERE id = ?", id)
db.exec(conn, "INSERT INTO users (name, email) VALUES (?, ?)", name, email)
```

### cache.*

In-memory key-value store backed by a `HashMap` with optional TTL using `std::time::Instant`.
Data is process-local — not shared across processes and not persistent.

```ash
cache.get(key)              # ?str
cache.set(key val)          # void
cache.setex(key val ttl)    # void — ttl in seconds
cache.del(key)              # void
cache.flush()               # void
```

### auth.*

> **Status: stubs.** These functions return a "not yet implemented" error.
> Full implementation (bcrypt + jsonwebtoken) is planned.

```ash
auth.jwt(payload secret)      # str
auth.verify(token secret)     # ?{str: any}
auth.hash(password)           # str
auth.check(password hash)     # bool
```

### mail.*

> **Status: stub.** Returns a "not yet implemented" error.
> Planned: `lettre` crate with `SMTP_HOST` / `SMTP_USER` / `SMTP_PASS` env vars.

```ash
mail.send(to subject body)    # void
mail.html(to subject html)    # void
```

### store.*

> **Status: stub.** Returns a "not yet implemented" error.
> Planned: local filesystem backend first, then S3-compatible via `STORE_URL`.

```ash
store.put(key data)           # void
store.get(key)                # ?str
store.del(key)                # void
store.url(key)                # str — public URL
store.list(prefix)            # [str]
```

### ai.*

Calls the **Anthropic API** (`api.anthropic.com`). Requires `ANTHROPIC_API_KEY` to be set
in the environment. Uses `claude-3-5-haiku-20241022` by default.

```ash
ai.complete(prompt)           # ?str — single-turn completion
ai.chat(messages)             # ?str — messages is [{role: str, content: str}]
ai.embed(text)                # [float] — stub
ai.similarity(a b)            # float — stub
ai.classify(text labels)      # str — stub
ai.moderate(text)             # bool — stub
```

Example:
```ash
key = env.require("ANTHROPIC_API_KEY")
reply = ai.complete("What is 2 + 2?") ?? "no response"
println(reply)
```

---

## 17. The two backends

### Interpreter (`ash run`)

A tree-walking interpreter over the AST. The `Arc<Mutex<HashMap>>` global environment ensures mutations inside functions are visible to callers with zero propagation overhead. Closure-captured locals are snapshotted at lambda creation time.

**Characteristics:**
- Zero build step — starts in under 10ms
- Ownership and borrow checking is not enforced (warnings only)
- Errors include line numbers and variable names
- Ideal for: development, scripting, REPL, tooling, CI checks

### Compiler (`ash build`)

The AST is run through the type checker to produce a fully-typed HIR (High-level IR). The HIR is then compiled to LLVM IR text (`.ll` file), which is passed to `clang` to produce a native binary.

**Pipeline:**

```
Source
  └── Lexer         tokens
       └── Parser   AST
            └── TypeChecker   HIR (typed, desugared)
                 └── Codegen   LLVM IR (.ll)
                      └── clang   native binary
```

**HIR desugaring:**
- `a |> f(b)` → `Call(f, [a, b])`
- `a ?? b` → `if a != none then unwrap(a) else b`
- `expr!` → early return on Err
- Lambdas are lifted to named functions with capture lists
- String interpolation is lowered to concatenation

**Key implementation notes:**
- Booleans use `i64` at ABI boundaries (function signatures) to avoid LLVM FastISel bugs; `i1` only for alloca storage
- Phi node predecessors are tracked via `cur_block` — updated on every emitted label — to correctly handle if-branches that contain loops
- User-defined function call sites use the signature registry to determine return types, not the calling context

**Characteristics:**
- Requires `clang` (tested with clang-18 and clang-20)
- Produces fast, self-contained native binaries via `ash_runtime.c` linked by clang
- No runtime, no GC, no startup overhead
- On the compute benchmark: faster than Go, 11.5x faster than Python

**Currently supported in codegen:**
- Arithmetic, comparisons, boolean logic
- Control flow: `if/else`, `while`, `for`
- Functions with explicit or inferred return types
- Recursion
- `match` on integers and union variants
- `math.*` calls
- `println` with int, float, string arguments
- String interpolation (lowered to `StrConcat` chains via `@ash_str_concat`)
- Heap lists (`[int]`) via `ash_list_new/push/get/len` from `ash_runtime.c`

**Not yet in codegen:**
- Heap maps (`{str: any}`)
- Closures / lambdas passed as values
- Stdlib calls other than `math.*` and `println`

---

## Quick reference card

```ash
# Variables
x = 5              # immutable
mut y = 0          # mutable
let z: int = 10    # explicit

# Functions
fn f(a b)          # inferred types
    a + b
fn g(a:int):int    # annotated
    a * 2

# Lambdas
double = x => x * 2
add    = (x y) => x + y

# Types
type Point
    x: float
    y: float
type Color = Red | Green | Blue

# Control
if cond
    a
else
    b

while cond
    body

for x in items
    body

match val
    Pattern => result
    _       => default

# Null safety
opt?.field          # safe navigation
opt ?? default      # fallback
expr!               # propagate error

# Pipeline
data |> filter(x => x > 0) |> map(x => x * 2)

# Collections
[1, 2, 3]                    # list
{"key": "val"}               # map
(1, "hello")                 # tuple
items.filter(x => x > 0)
items.map(x => x * 2)
items.reduce((a x) => a + x, 0)
```
