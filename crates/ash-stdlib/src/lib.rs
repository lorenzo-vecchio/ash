//! ash-stdlib
//! Defines all standard library namespaces available to Ash programs.
//! This crate has no external dependencies — it is pure Rust.
//!
//! Namespaces:
//!   Core (no prefix):  print, println, fmt, abs, min, max, int, float, str, bool
//!   math.*            floor, ceil, round, sqrt, pow, log, sin, cos, clamp
//!   str.*             (methods on string values)
//!   list.*            (methods on list values)
//!   map.*             (methods on map values)
//!   file.*            read, write, append, exists, ls, rm
//!   http.*            get, post, put, del, fetch
//!   json.*            parse, str
//!   re.*              match, find, findall, replace
//!   env.*             get, require, all
//!   go.*              spawn, wait, all, race, sleep, chan
//!   db.*              connect, query, exec, tx
//!   cache.*           get, set, del, flush
//!   queue.*           push, pop, sub
//!   auth.*            jwt, verify, hash, check
//!   mail.*            send
//!   store.*           put, get, del, url
//!   ai.*              complete, embed, similarity, classify

// ─── Function signature descriptor ───────────────────────────────────────────

/// Describes a stdlib function for tooling, documentation, and type checking.
#[derive(Debug, Clone)]
pub struct StdlibFn {
    pub namespace: &'static str,
    pub name: &'static str,
    pub params: &'static [(&'static str, &'static str)], // (name, type)
    pub ret: &'static str,
    pub doc: &'static str,
}

impl StdlibFn {
    pub fn full_name(&self) -> String {
        if self.namespace.is_empty() {
            self.name.to_string()
        } else {
            format!("{}.{}", self.namespace, self.name)
        }
    }
}

// ─── Stdlib registry ──────────────────────────────────────────────────────────

pub fn all_functions() -> Vec<StdlibFn> {
    let mut fns = vec![];
    fns.extend(core_fns());
    fns.extend(math_fns());
    fns.extend(file_fns());
    fns.extend(http_fns());
    fns.extend(json_fns());
    fns.extend(re_fns());
    fns.extend(env_fns());
    fns.extend(go_fns());
    fns.extend(db_fns());
    fns.extend(cache_fns());
    fns.extend(queue_fns());
    fns.extend(auth_fns());
    fns.extend(mail_fns());
    fns.extend(store_fns());
    fns.extend(ai_fns());
    fns
}

pub fn lookup(namespace: &str, name: &str) -> Option<StdlibFn> {
    all_functions()
        .into_iter()
        .find(|f| f.namespace == namespace && f.name == name)
}

pub fn lookup_full(full_name: &str) -> Option<StdlibFn> {
    if let Some(dot) = full_name.rfind('.') {
        lookup(&full_name[..dot], &full_name[dot + 1..])
    } else {
        lookup("", full_name)
    }
}

// ─── Core ─────────────────────────────────────────────────────────────────────

fn core_fns() -> Vec<StdlibFn> {
    vec![
        StdlibFn {
            namespace: "",
            name: "print",
            params: &[("x", "any")],
            ret: "void",
            doc: "Print value without newline",
        },
        StdlibFn {
            namespace: "",
            name: "println",
            params: &[("x", "any")],
            ret: "void",
            doc: "Print value with newline",
        },
        StdlibFn {
            namespace: "",
            name: "read",
            params: &[],
            ret: "str",
            doc: "Read a line from stdin",
        },
        StdlibFn {
            namespace: "",
            name: "fmt",
            params: &[("tmpl", "str"), ("args", "any...")],
            ret: "str",
            doc: "Format string with {} placeholders",
        },
        StdlibFn {
            namespace: "",
            name: "int",
            params: &[("x", "any")],
            ret: "int",
            doc: "Convert to int",
        },
        StdlibFn {
            namespace: "",
            name: "float",
            params: &[("x", "any")],
            ret: "float",
            doc: "Convert to float",
        },
        StdlibFn {
            namespace: "",
            name: "str",
            params: &[("x", "any")],
            ret: "str",
            doc: "Convert to string",
        },
        StdlibFn {
            namespace: "",
            name: "bool",
            params: &[("x", "any")],
            ret: "bool",
            doc: "Convert to bool",
        },
        StdlibFn {
            namespace: "",
            name: "abs",
            params: &[("x", "num")],
            ret: "num",
            doc: "Absolute value",
        },
        StdlibFn {
            namespace: "",
            name: "min",
            params: &[("a", "T"), ("b", "T")],
            ret: "T",
            doc: "Minimum of two values",
        },
        StdlibFn {
            namespace: "",
            name: "max",
            params: &[("a", "T"), ("b", "T")],
            ret: "T",
            doc: "Maximum of two values",
        },
        StdlibFn {
            namespace: "",
            name: "clamp",
            params: &[("x", "T"), ("lo", "T"), ("hi", "T")],
            ret: "T",
            doc: "Clamp x between lo and hi",
        },
        StdlibFn {
            namespace: "",
            name: "filter",
            params: &[("l", "[T]"), ("f", "T=>bool")],
            ret: "[T]",
            doc: "Keep elements where f returns true",
        },
        StdlibFn {
            namespace: "",
            name: "map",
            params: &[("l", "[T]"), ("f", "T=>U")],
            ret: "[U]",
            doc: "Transform each element",
        },
        StdlibFn {
            namespace: "",
            name: "reduce",
            params: &[("l", "[T]"), ("f", "(U T)=>U"), ("init", "U")],
            ret: "U",
            doc: "Fold list to single value",
        },
        StdlibFn {
            namespace: "",
            name: "zip",
            params: &[("a", "[T]"), ("b", "[U]")],
            ret: "[(T U)]",
            doc: "Pair elements from two lists",
        },
        StdlibFn {
            namespace: "",
            name: "flat",
            params: &[("l", "[[T]]")],
            ret: "[T]",
            doc: "Flatten one level of nesting",
        },
        StdlibFn {
            namespace: "",
            name: "any",
            params: &[("l", "[T]"), ("f", "T=>bool")],
            ret: "bool",
            doc: "True if any element matches",
        },
        StdlibFn {
            namespace: "",
            name: "all",
            params: &[("l", "[T]"), ("f", "T=>bool")],
            ret: "bool",
            doc: "True if all elements match",
        },
        StdlibFn {
            namespace: "",
            name: "panic",
            params: &[("msg", "str")],
            ret: "void",
            doc: "Terminate with an error message",
        },
    ]
}

// ─── Math ─────────────────────────────────────────────────────────────────────

fn math_fns() -> Vec<StdlibFn> {
    vec![
        StdlibFn {
            namespace: "math",
            name: "floor",
            params: &[("x", "float")],
            ret: "float",
            doc: "Round down",
        },
        StdlibFn {
            namespace: "math",
            name: "ceil",
            params: &[("x", "float")],
            ret: "float",
            doc: "Round up",
        },
        StdlibFn {
            namespace: "math",
            name: "round",
            params: &[("x", "float")],
            ret: "float",
            doc: "Round to nearest",
        },
        StdlibFn {
            namespace: "math",
            name: "sqrt",
            params: &[("x", "float")],
            ret: "float",
            doc: "Square root",
        },
        StdlibFn {
            namespace: "math",
            name: "pow",
            params: &[("x", "float"), ("e", "float")],
            ret: "float",
            doc: "x raised to e",
        },
        StdlibFn {
            namespace: "math",
            name: "log",
            params: &[("x", "float")],
            ret: "float",
            doc: "Natural logarithm",
        },
        StdlibFn {
            namespace: "math",
            name: "log2",
            params: &[("x", "float")],
            ret: "float",
            doc: "Base-2 logarithm",
        },
        StdlibFn {
            namespace: "math",
            name: "log10",
            params: &[("x", "float")],
            ret: "float",
            doc: "Base-10 logarithm",
        },
        StdlibFn {
            namespace: "math",
            name: "sin",
            params: &[("x", "float")],
            ret: "float",
            doc: "Sine",
        },
        StdlibFn {
            namespace: "math",
            name: "cos",
            params: &[("x", "float")],
            ret: "float",
            doc: "Cosine",
        },
        StdlibFn {
            namespace: "math",
            name: "tan",
            params: &[("x", "float")],
            ret: "float",
            doc: "Tangent",
        },
        StdlibFn {
            namespace: "math",
            name: "pi",
            params: &[],
            ret: "float",
            doc: "π constant",
        },
        StdlibFn {
            namespace: "math",
            name: "e",
            params: &[],
            ret: "float",
            doc: "Euler's number",
        },
    ]
}

// ─── File ─────────────────────────────────────────────────────────────────────

fn file_fns() -> Vec<StdlibFn> {
    vec![
        StdlibFn {
            namespace: "file",
            name: "read",
            params: &[("path", "str")],
            ret: "?str",
            doc: "Read file contents, None if missing",
        },
        StdlibFn {
            namespace: "file",
            name: "write",
            params: &[("path", "str"), ("data", "str")],
            ret: "void",
            doc: "Write data to file, overwriting",
        },
        StdlibFn {
            namespace: "file",
            name: "append",
            params: &[("path", "str"), ("data", "str")],
            ret: "void",
            doc: "Append data to file",
        },
        StdlibFn {
            namespace: "file",
            name: "exists",
            params: &[("path", "str")],
            ret: "bool",
            doc: "True if file exists",
        },
        StdlibFn {
            namespace: "file",
            name: "ls",
            params: &[("dir", "str")],
            ret: "[str]",
            doc: "List files in directory",
        },
        StdlibFn {
            namespace: "file",
            name: "rm",
            params: &[("path", "str")],
            ret: "void",
            doc: "Delete file",
        },
        StdlibFn {
            namespace: "file",
            name: "mkdir",
            params: &[("path", "str")],
            ret: "void",
            doc: "Create directory",
        },
        StdlibFn {
            namespace: "file",
            name: "mv",
            params: &[("src", "str"), ("dst", "str")],
            ret: "void",
            doc: "Move/rename file",
        },
        StdlibFn {
            namespace: "file",
            name: "cp",
            params: &[("src", "str"), ("dst", "str")],
            ret: "void",
            doc: "Copy file",
        },
    ]
}

// ─── HTTP ─────────────────────────────────────────────────────────────────────

fn http_fns() -> Vec<StdlibFn> {
    vec![
        StdlibFn {
            namespace: "http",
            name: "get",
            params: &[("url", "str")],
            ret: "?str",
            doc: "GET request, returns body",
        },
        StdlibFn {
            namespace: "http",
            name: "post",
            params: &[("url", "str"), ("body", "str")],
            ret: "?str",
            doc: "POST request",
        },
        StdlibFn {
            namespace: "http",
            name: "put",
            params: &[("url", "str"), ("body", "str")],
            ret: "?str",
            doc: "PUT request",
        },
        StdlibFn {
            namespace: "http",
            name: "del",
            params: &[("url", "str")],
            ret: "?str",
            doc: "DELETE request",
        },
        StdlibFn {
            namespace: "http",
            name: "patch",
            params: &[("url", "str"), ("body", "str")],
            ret: "?str",
            doc: "PATCH request",
        },
        StdlibFn {
            namespace: "http",
            name: "fetch",
            params: &[("url", "str"), ("opts", "{str:any}")],
            ret: "Response",
            doc: "Full request with options",
        },
    ]
}

// ─── JSON ─────────────────────────────────────────────────────────────────────

fn json_fns() -> Vec<StdlibFn> {
    vec![
        StdlibFn {
            namespace: "json",
            name: "parse",
            params: &[("s", "str")],
            ret: "?any",
            doc: "Parse JSON string to value",
        },
        StdlibFn {
            namespace: "json",
            name: "str",
            params: &[("x", "any")],
            ret: "str",
            doc: "Serialize value to JSON string",
        },
        StdlibFn {
            namespace: "json",
            name: "pretty",
            params: &[("x", "any")],
            ret: "str",
            doc: "Pretty-print JSON",
        },
    ]
}

// ─── Regex ────────────────────────────────────────────────────────────────────

fn re_fns() -> Vec<StdlibFn> {
    vec![
        StdlibFn {
            namespace: "re",
            name: "match",
            params: &[("pattern", "str"), ("s", "str")],
            ret: "bool",
            doc: "True if pattern matches",
        },
        StdlibFn {
            namespace: "re",
            name: "find",
            params: &[("pattern", "str"), ("s", "str")],
            ret: "?str",
            doc: "First match or None",
        },
        StdlibFn {
            namespace: "re",
            name: "findall",
            params: &[("pattern", "str"), ("s", "str")],
            ret: "[str]",
            doc: "All matches",
        },
        StdlibFn {
            namespace: "re",
            name: "replace",
            params: &[("pattern", "str"), ("s", "str"), ("repl", "str")],
            ret: "str",
            doc: "Replace all matches",
        },
        StdlibFn {
            namespace: "re",
            name: "split",
            params: &[("pattern", "str"), ("s", "str")],
            ret: "[str]",
            doc: "Split on pattern",
        },
    ]
}

// ─── Env ──────────────────────────────────────────────────────────────────────

fn env_fns() -> Vec<StdlibFn> {
    vec![
        StdlibFn {
            namespace: "env",
            name: "get",
            params: &[("key", "str")],
            ret: "?str",
            doc: "Get env var or None",
        },
        StdlibFn {
            namespace: "env",
            name: "require",
            params: &[("key", "str")],
            ret: "str",
            doc: "Get env var or panic",
        },
        StdlibFn {
            namespace: "env",
            name: "all",
            params: &[],
            ret: "{str:str}",
            doc: "All env vars as map",
        },
        StdlibFn {
            namespace: "env",
            name: "set",
            params: &[("key", "str"), ("val", "str")],
            ret: "void",
            doc: "Set env var",
        },
    ]
}

// ─── Concurrency ─────────────────────────────────────────────────────────────

fn go_fns() -> Vec<StdlibFn> {
    vec![
        StdlibFn {
            namespace: "go",
            name: "spawn",
            params: &[("f", "()=>T")],
            ret: "Task[T]",
            doc: "Spawn concurrent task",
        },
        StdlibFn {
            namespace: "go",
            name: "wait",
            params: &[("t", "Task[T]")],
            ret: "T",
            doc: "Wait for task result",
        },
        StdlibFn {
            namespace: "go",
            name: "all",
            params: &[("ts", "[Task[T]]")],
            ret: "[T]",
            doc: "Wait for all tasks",
        },
        StdlibFn {
            namespace: "go",
            name: "race",
            params: &[("ts", "[Task[T]]")],
            ret: "T",
            doc: "First task to finish",
        },
        StdlibFn {
            namespace: "go",
            name: "sleep",
            params: &[("ms", "int")],
            ret: "void",
            doc: "Sleep for ms milliseconds",
        },
        StdlibFn {
            namespace: "go",
            name: "chan",
            params: &[],
            ret: "Chan[T]",
            doc: "Create a channel",
        },
    ]
}

// ─── Database ─────────────────────────────────────────────────────────────────

fn db_fns() -> Vec<StdlibFn> {
    vec![
        StdlibFn {
            namespace: "db",
            name: "connect",
            params: &[("url", "str")],
            ret: "Connection",
            doc: "Connect to a database",
        },
        StdlibFn {
            namespace: "db",
            name: "query",
            params: &[("conn", "Connection"), ("sql", "str"), ("args", "any...")],
            ret: "[{str:any}]",
            doc: "Execute query, return rows",
        },
        StdlibFn {
            namespace: "db",
            name: "exec",
            params: &[("conn", "Connection"), ("sql", "str"), ("args", "any...")],
            ret: "int",
            doc: "Execute statement, return row count",
        },
        StdlibFn {
            namespace: "db",
            name: "tx",
            params: &[("conn", "Connection"), ("f", "Connection=>T")],
            ret: "T",
            doc: "Run in transaction, auto-rollback on error",
        },
        StdlibFn {
            namespace: "db",
            name: "close",
            params: &[("conn", "Connection")],
            ret: "void",
            doc: "Close connection",
        },
    ]
}

// ─── Cache ────────────────────────────────────────────────────────────────────

fn cache_fns() -> Vec<StdlibFn> {
    vec![
        StdlibFn {
            namespace: "cache",
            name: "get",
            params: &[("key", "str")],
            ret: "?str",
            doc: "Get cached value",
        },
        StdlibFn {
            namespace: "cache",
            name: "set",
            params: &[("key", "str"), ("val", "str")],
            ret: "void",
            doc: "Set cached value",
        },
        StdlibFn {
            namespace: "cache",
            name: "setex",
            params: &[("key", "str"), ("val", "str"), ("ttl", "int")],
            ret: "void",
            doc: "Set with TTL in seconds",
        },
        StdlibFn {
            namespace: "cache",
            name: "del",
            params: &[("key", "str")],
            ret: "void",
            doc: "Delete cached value",
        },
        StdlibFn {
            namespace: "cache",
            name: "flush",
            params: &[],
            ret: "void",
            doc: "Clear all cached values",
        },
    ]
}

// ─── Queue ────────────────────────────────────────────────────────────────────

fn queue_fns() -> Vec<StdlibFn> {
    vec![
        StdlibFn {
            namespace: "queue",
            name: "push",
            params: &[("name", "str"), ("msg", "str")],
            ret: "void",
            doc: "Push message to queue",
        },
        StdlibFn {
            namespace: "queue",
            name: "pop",
            params: &[("name", "str")],
            ret: "?str",
            doc: "Pop message, None if empty",
        },
        StdlibFn {
            namespace: "queue",
            name: "sub",
            params: &[("name", "str"), ("f", "str=>void")],
            ret: "void",
            doc: "Subscribe to queue",
        },
        StdlibFn {
            namespace: "queue",
            name: "len",
            params: &[("name", "str")],
            ret: "int",
            doc: "Queue length",
        },
    ]
}

// ─── Auth ─────────────────────────────────────────────────────────────────────

fn auth_fns() -> Vec<StdlibFn> {
    vec![
        StdlibFn {
            namespace: "auth",
            name: "jwt",
            params: &[("payload", "{str:any}"), ("secret", "str")],
            ret: "str",
            doc: "Sign a JWT",
        },
        StdlibFn {
            namespace: "auth",
            name: "verify",
            params: &[("token", "str"), ("secret", "str")],
            ret: "?{str:any}",
            doc: "Verify JWT, None if invalid",
        },
        StdlibFn {
            namespace: "auth",
            name: "hash",
            params: &[("password", "str")],
            ret: "str",
            doc: "Bcrypt hash a password",
        },
        StdlibFn {
            namespace: "auth",
            name: "check",
            params: &[("password", "str"), ("hash", "str")],
            ret: "bool",
            doc: "Verify password against hash",
        },
    ]
}

// ─── Mail ─────────────────────────────────────────────────────────────────────

fn mail_fns() -> Vec<StdlibFn> {
    vec![
        StdlibFn {
            namespace: "mail",
            name: "send",
            params: &[("to", "str"), ("subject", "str"), ("body", "str")],
            ret: "void",
            doc: "Send plain text email",
        },
        StdlibFn {
            namespace: "mail",
            name: "html",
            params: &[("to", "str"), ("subject", "str"), ("body", "str")],
            ret: "void",
            doc: "Send HTML email",
        },
    ]
}

// ─── Store (blob) ────────────────────────────────────────────────────────────

fn store_fns() -> Vec<StdlibFn> {
    vec![
        StdlibFn {
            namespace: "store",
            name: "put",
            params: &[("key", "str"), ("data", "str")],
            ret: "?str",
            doc: "Upload data, returns URL",
        },
        StdlibFn {
            namespace: "store",
            name: "get",
            params: &[("key", "str")],
            ret: "?str",
            doc: "Download data",
        },
        StdlibFn {
            namespace: "store",
            name: "del",
            params: &[("key", "str")],
            ret: "void",
            doc: "Delete object",
        },
        StdlibFn {
            namespace: "store",
            name: "url",
            params: &[("key", "str")],
            ret: "?str",
            doc: "Get public URL",
        },
        StdlibFn {
            namespace: "store",
            name: "list",
            params: &[("prefix", "str")],
            ret: "[str]",
            doc: "List keys with prefix",
        },
    ]
}

// ─── AI ───────────────────────────────────────────────────────────────────────

fn ai_fns() -> Vec<StdlibFn> {
    vec![
        StdlibFn {
            namespace: "ai",
            name: "complete",
            params: &[("prompt", "str")],
            ret: "?str",
            doc: "LLM text completion",
        },
        StdlibFn {
            namespace: "ai",
            name: "chat",
            params: &[("messages", "[{str:str}]")],
            ret: "?str",
            doc: "LLM chat completion",
        },
        StdlibFn {
            namespace: "ai",
            name: "embed",
            params: &[("text", "str")],
            ret: "[float]",
            doc: "Text embedding vector",
        },
        StdlibFn {
            namespace: "ai",
            name: "similarity",
            params: &[("a", "[float]"), ("b", "[float]")],
            ret: "float",
            doc: "Cosine similarity",
        },
        StdlibFn {
            namespace: "ai",
            name: "classify",
            params: &[("text", "str"), ("labels", "[str]")],
            ret: "str",
            doc: "Zero-shot classification",
        },
        StdlibFn {
            namespace: "ai",
            name: "moderate",
            params: &[("text", "str")],
            ret: "bool",
            doc: "Content moderation check",
        },
    ]
}

// ─── Runtime interpreter stubs ────────────────────────────────────────────────
// These functions implement stdlib behavior for the interpreter.
// Each returns a Result<String, String> — the interpreter maps these to Values.

pub struct RuntimeContext {
    pub env_vars: std::collections::HashMap<String, String>,
}

impl Default for RuntimeContext {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeContext {
    pub fn new() -> Self {
        RuntimeContext {
            env_vars: std::env::vars().collect(),
        }
    }
}

/// Math operations — pure, no IO
pub mod math {
    pub fn floor(x: f64) -> f64 {
        x.floor()
    }
    pub fn ceil(x: f64) -> f64 {
        x.ceil()
    }
    pub fn round(x: f64) -> f64 {
        x.round()
    }
    pub fn sqrt(x: f64) -> f64 {
        x.sqrt()
    }
    pub fn pow(x: f64, e: f64) -> f64 {
        x.powf(e)
    }
    pub fn log(x: f64) -> f64 {
        x.ln()
    }
    pub fn log2(x: f64) -> f64 {
        x.log2()
    }
    pub fn log10(x: f64) -> f64 {
        x.log10()
    }
    pub fn sin(x: f64) -> f64 {
        x.sin()
    }
    pub fn cos(x: f64) -> f64 {
        x.cos()
    }
    pub fn tan(x: f64) -> f64 {
        x.tan()
    }
    pub fn pi() -> f64 {
        std::f64::consts::PI
    }
    pub fn e() -> f64 {
        std::f64::consts::E
    }
    pub fn clamp(x: f64, lo: f64, hi: f64) -> f64 {
        x.clamp(lo, hi)
    }
    pub fn clamp_int(x: i64, lo: i64, hi: i64) -> i64 {
        x.clamp(lo, hi)
    }
}

/// Environment variable access
pub mod env {
    pub fn get(key: &str) -> Option<String> {
        std::env::var(key).ok()
    }
    pub fn require(key: &str) -> Result<String, String> {
        std::env::var(key).map_err(|_| format!("required env var '{key}' is not set"))
    }
    pub fn all() -> Vec<(String, String)> {
        std::env::vars().collect()
    }
}

/// File operations
pub mod file {
    use std::path::Path;

    pub fn read(path: &str) -> Option<String> {
        std::fs::read_to_string(path).ok()
    }
    pub fn write(path: &str, data: &str) -> Result<(), String> {
        std::fs::write(path, data).map_err(|e| e.to_string())
    }
    pub fn append(path: &str, data: &str) -> Result<(), String> {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(path)
            .map_err(|e| e.to_string())?;
        f.write_all(data.as_bytes()).map_err(|e| e.to_string())
    }
    pub fn exists(path: &str) -> bool {
        Path::new(path).exists()
    }
    pub fn ls(dir: &str) -> Result<Vec<String>, String> {
        let entries = std::fs::read_dir(dir).map_err(|e| e.to_string())?;
        Ok(entries
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect())
    }
    pub fn rm(path: &str) -> Result<(), String> {
        if Path::new(path).is_dir() {
            std::fs::remove_dir_all(path).map_err(|e| e.to_string())
        } else {
            std::fs::remove_file(path).map_err(|e| e.to_string())
        }
    }
    pub fn mkdir(path: &str) -> Result<(), String> {
        std::fs::create_dir_all(path).map_err(|e| e.to_string())
    }
}

/// JSON — requires serde_json when available, falls back to basic impl
pub mod json {
    pub fn to_string_basic(val: &str) -> String {
        // Basic escaping for strings
        format!("\"{}\"", val.replace('"', "\\\""))
    }
    pub fn is_valid(s: &str) -> bool {
        // Very basic validation — just check it starts/ends with {} or [] or is a scalar
        let s = s.trim();
        (s.starts_with('{') && s.ends_with('}'))
            || (s.starts_with('[') && s.ends_with(']'))
            || s.starts_with('"')
            || s.parse::<f64>().is_ok()
            || s == "true"
            || s == "false"
            || s == "null"
    }
}

/// Simple string interpolation resolver (used at runtime)
pub fn interpolate(template: &str, vars: &std::collections::HashMap<String, String>) -> String {
    let mut result = String::new();
    let mut chars = template.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '{' {
            let mut name = String::new();
            for inner in chars.by_ref() {
                if inner == '}' {
                    break;
                }
                name.push(inner);
            }
            if let Some(val) = vars.get(&name) {
                result.push_str(val);
            } else {
                result.push('{');
                result.push_str(&name);
                result.push('}');
            }
        } else {
            result.push(c);
        }
    }
    result
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_functions_non_empty() {
        assert!(!all_functions().is_empty());
    }

    #[test]
    fn test_core_functions_present() {
        assert!(lookup("", "println").is_some());
        assert!(lookup("", "print").is_some());
        assert!(lookup("", "int").is_some());
        assert!(lookup("", "float").is_some());
        assert!(lookup("", "str").is_some());
        assert!(lookup("", "abs").is_some());
        assert!(lookup("", "min").is_some());
        assert!(lookup("", "max").is_some());
        assert!(lookup("", "filter").is_some());
        assert!(lookup("", "map").is_some());
    }

    #[test]
    fn test_namespaced_functions_present() {
        assert!(lookup("math", "sqrt").is_some());
        assert!(lookup("file", "read").is_some());
        assert!(lookup("http", "get").is_some());
        assert!(lookup("json", "parse").is_some());
        assert!(lookup("re", "match").is_some());
        assert!(lookup("env", "get").is_some());
        assert!(lookup("go", "spawn").is_some());
        assert!(lookup("db", "connect").is_some());
        assert!(lookup("cache", "get").is_some());
        assert!(lookup("queue", "push").is_some());
        assert!(lookup("auth", "jwt").is_some());
        assert!(lookup("mail", "send").is_some());
        assert!(lookup("store", "put").is_some());
        assert!(lookup("ai", "complete").is_some());
    }

    #[test]
    fn test_lookup_full_name() {
        assert!(lookup_full("math.sqrt").is_some());
        assert!(lookup_full("println").is_some());
        assert!(lookup_full("ai.embed").is_some());
        assert!(lookup_full("nonexistent").is_none());
        assert!(lookup_full("fake.fn").is_none());
    }

    #[test]
    fn test_full_name_format() {
        let f = lookup("math", "sqrt").unwrap();
        assert_eq!(f.full_name(), "math.sqrt");
        let g = lookup("", "println").unwrap();
        assert_eq!(g.full_name(), "println");
    }

    #[test]
    fn test_function_count() {
        // Should have at least 60 functions
        assert!(all_functions().len() >= 60);
    }

    #[test]
    fn test_math_floor() {
        assert_eq!(math::floor(3.7), 3.0);
        assert_eq!(math::floor(-3.2), -4.0);
    }

    #[test]
    fn test_math_ceil() {
        assert_eq!(math::ceil(3.2), 4.0);
        assert_eq!(math::ceil(-3.7), -3.0);
    }

    #[test]
    fn test_math_round() {
        assert_eq!(math::round(3.5), 4.0);
        assert_eq!(math::round(3.4), 3.0);
    }

    #[test]
    fn test_math_sqrt() {
        assert_eq!(math::sqrt(4.0), 2.0);
        assert_eq!(math::sqrt(9.0), 3.0);
    }

    #[test]
    fn test_math_pow() {
        assert_eq!(math::pow(2.0, 10.0), 1024.0);
        assert_eq!(math::pow(3.0, 2.0), 9.0);
    }

    #[test]
    fn test_math_trig() {
        let pi = math::pi();
        assert!((math::sin(pi) - 0.0).abs() < 1e-10);
        assert!((math::cos(0.0) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_math_pi_e() {
        assert!((math::pi() - std::f64::consts::PI).abs() < 1e-15);
        assert!((math::e() - std::f64::consts::E).abs() < 1e-15);
    }

    #[test]
    fn test_math_clamp() {
        assert_eq!(math::clamp(5.0, 0.0, 10.0), 5.0);
        assert_eq!(math::clamp(-5.0, 0.0, 10.0), 0.0);
        assert_eq!(math::clamp(15.0, 0.0, 10.0), 10.0);
    }

    #[test]
    fn test_env_get_missing() {
        assert!(env::get("__ASH_NO_SUCH_VAR__").is_none());
    }

    #[test]
    fn test_env_require_missing() {
        assert!(env::require("__ASH_NO_SUCH_VAR__").is_err());
    }

    #[test]
    fn test_file_exists_missing() {
        assert!(!file::exists("/tmp/__ash_no_such_file_xyz__"));
    }

    #[test]
    fn test_file_write_read_rm() {
        let path = "/tmp/__ash_test_stdlib__.txt";
        file::write(path, "hello ash").expect("write");
        assert!(file::exists(path));
        let content = file::read(path).expect("read");
        assert_eq!(content, "hello ash");
        file::rm(path).expect("rm");
        assert!(!file::exists(path));
    }

    #[test]
    fn test_file_append() {
        let path = "/tmp/__ash_test_append__.txt";
        file::write(path, "line1\n").expect("write");
        file::append(path, "line2\n").expect("append");
        let content = file::read(path).expect("read");
        assert!(content.contains("line1"));
        assert!(content.contains("line2"));
        file::rm(path).ok();
    }

    #[test]
    fn test_file_ls() {
        let entries = file::ls("/tmp").expect("ls");
        assert!(!entries.is_empty());
    }

    #[test]
    fn test_file_mkdir() {
        let path = "/tmp/__ash_test_dir__";
        file::mkdir(path).expect("mkdir");
        assert!(file::exists(path));
        file::rm(path).ok();
    }

    #[test]
    fn test_json_is_valid() {
        assert!(json::is_valid("{\"key\": \"value\"}"));
        assert!(json::is_valid("[1, 2, 3]"));
        assert!(json::is_valid("\"hello\""));
        assert!(json::is_valid("42"));
        assert!(json::is_valid("true"));
        assert!(!json::is_valid("invalid json {{{"));
    }

    #[test]
    fn test_interpolate_basic() {
        let mut vars = std::collections::HashMap::new();
        vars.insert("name".to_string(), "Lorenzo".to_string());
        let result = interpolate("hello {name}!", &vars);
        assert_eq!(result, "hello Lorenzo!");
    }

    #[test]
    fn test_interpolate_multiple() {
        let mut vars = std::collections::HashMap::new();
        vars.insert("lang".to_string(), "Ash".to_string());
        vars.insert("ver".to_string(), "0.1".to_string());
        let result = interpolate("{lang} version {ver}", &vars);
        assert_eq!(result, "Ash version 0.1");
    }

    #[test]
    fn test_interpolate_missing_var() {
        let vars = std::collections::HashMap::new();
        let result = interpolate("hello {missing}", &vars);
        assert_eq!(result, "hello {missing}");
    }

    #[test]
    fn test_math_log() {
        assert!((math::log(std::f64::consts::E) - 1.0).abs() < 1e-10);
        assert!((math::log2(8.0) - 3.0).abs() < 1e-10);
        assert!((math::log10(1000.0) - 3.0).abs() < 1e-10);
    }
}
