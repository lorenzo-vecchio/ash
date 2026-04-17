//! Ash Interpreter
//! Tree-walking interpreter. Evaluates AST nodes directly.
//! Used for `ash run` — fast startup, great error messages, no compilation step.

use ash_parser::ast::*;
use std::collections::HashMap;
use std::fmt;

// --- Value --------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    List(Vec<Value>),
    Map(Vec<(Value, Value)>),
    Tuple(Vec<Value>),
    Option(Option<Box<Value>>),
    // Result variants
    Ok(Box<Value>),
    Err(Box<Value>),
    // User-defined variant: Variant name + payload
    Variant(String, Vec<Value>),
    // Struct instance: type name + fields
    Struct(String, HashMap<String, Value>),
    // Callable
    Fn(FnValue),
    // Unit / void
    Unit,
}

#[derive(Debug, Clone)]
pub struct FnValue {
    pub name: Option<String>,
    pub params: Vec<Param>,
    pub body: FnBody,
    pub closure: Env,
}

pub enum FnBody {
    Ast(Vec<Stmt>),
    Native(std::sync::Arc<dyn Fn(&[Value]) -> InterpResult<Value> + Send + Sync>),
}

impl std::fmt::Debug for FnBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FnBody::Ast(_) => write!(f, "FnBody::Ast(...)"),
            FnBody::Native(_) => write!(f, "FnBody::Native(...)"),
        }
    }
}

impl Clone for FnBody {
    fn clone(&self) -> Self {
        match self {
            FnBody::Ast(stmts) => FnBody::Ast(stmts.clone()),
            FnBody::Native(arc) => FnBody::Native(arc.clone()),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{n}"),
            Value::Float(n) => write!(f, "{n}"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Str(s) => write!(f, "{s}"),
            Value::Unit => write!(f, "()"),
            Value::Option(None) => write!(f, "none"),
            Value::Option(Some(v)) => write!(f, "some({v})"),
            Value::Ok(v) => write!(f, "Ok({v})"),
            Value::Err(v) => write!(f, "Err({v})"),
            Value::Variant(name, fields) => {
                if fields.is_empty() {
                    write!(f, "{name}")
                } else {
                    let fs: Vec<_> = fields.iter().map(|v| v.to_string()).collect();
                    write!(f, "{name}({})", fs.join(", "))
                }
            }
            Value::Struct(name, fields) => {
                let fs: Vec<_> = fields.iter().map(|(k, v)| format!("{k}: {v}")).collect();
                write!(f, "{name} {{ {} }}", fs.join(", "))
            }
            Value::List(items) => {
                let ss: Vec<_> = items.iter().map(|v| v.to_string()).collect();
                write!(f, "[{}]", ss.join(", "))
            }
            Value::Map(pairs) => {
                let ss: Vec<_> = pairs.iter().map(|(k, v)| format!("{k}: {v}")).collect();
                write!(f, "{{{}}}", ss.join(", "))
            }
            Value::Tuple(items) => {
                let ss: Vec<_> = items.iter().map(|v| v.to_string()).collect();
                write!(f, "({})", ss.join(", "))
            }
            Value::Fn(fv) => write!(f, "<fn {}>", fv.name.as_deref().unwrap_or("anonymous")),
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Unit, Value::Unit) => true,
            (Value::Option(a), Value::Option(b)) => a == b,
            (Value::Ok(a), Value::Ok(b)) => a == b,
            (Value::Err(a), Value::Err(b)) => a == b,
            (Value::List(a), Value::List(b)) => a == b,
            (Value::Tuple(a), Value::Tuple(b)) => a == b,
            (Value::Variant(na, fa), Value::Variant(nb, fb)) => na == nb && fa == fb,
            _ => false,
        }
    }
}

// --- Environment -------------------------------------------------------------

use std::sync::Arc as Rc;
use std::sync::Mutex as RefCell;

#[derive(Clone)]
pub struct Env {
    globals: Rc<RefCell<HashMap<String, Value>>>,
    locals: Vec<HashMap<String, Value>>,
}

impl std::fmt::Debug for Env {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Env({} local frames)", self.locals.len())
    }
}

impl Default for Env {
    fn default() -> Self {
        Env {
            globals: Rc::new(RefCell::new(HashMap::new())),
            locals: vec![],
        }
    }
}

impl Env {
    pub fn new() -> Self {
        let mut e = Env {
            globals: Rc::new(RefCell::new(HashMap::new())),
            locals: vec![],
        };
        e.register_stdlib();
        e
    }

    /// Create a child scope that shares globals but gets fresh locals.
    pub fn child(&self) -> Self {
        Env {
            globals: Rc::clone(&self.globals),
            locals: vec![],
        }
    }

    pub fn push(&mut self) {
        self.locals.push(HashMap::new());
    }
    pub fn pop(&mut self) {
        self.locals.pop();
    }

    pub fn get(&self, name: &str) -> Option<Value> {
        for frame in self.locals.iter().rev() {
            if let Some(v) = frame.get(name) {
                return Some(v.clone());
            }
        }
        self.globals.lock().unwrap().get(name).cloned()
    }

    pub fn set(&mut self, name: &str, val: Value) {
        for frame in self.locals.iter_mut().rev() {
            if frame.contains_key(name) {
                frame.insert(name.to_string(), val);
                return;
            }
        }
        if self.globals.lock().unwrap().contains_key(name) {
            self.globals.lock().unwrap().insert(name.to_string(), val);
            return;
        }
        if let Some(frame) = self.locals.last_mut() {
            frame.insert(name.to_string(), val);
        } else {
            self.globals.lock().unwrap().insert(name.to_string(), val);
        }
    }

    pub fn define(&mut self, name: &str, val: Value) {
        if let Some(frame) = self.locals.last_mut() {
            frame.insert(name.to_string(), val);
        } else {
            self.globals.lock().unwrap().insert(name.to_string(), val);
        }
    }
    fn register_stdlib(&mut self) {
        // println
        self.define(
            "println",
            Value::Fn(FnValue {
                name: Some("println".into()),
                params: vec![],
                body: FnBody::Native(std::sync::Arc::new(|args| {
                    let s = args
                        .iter()
                        .map(|v| v.to_string())
                        .collect::<Vec<_>>()
                        .join(" ");
                    println!("{s}");
                    Ok(Value::Unit)
                })),
                closure: Env::default(),
            }),
        );
        // print
        self.define(
            "print",
            Value::Fn(FnValue {
                name: Some("print".into()),
                params: vec![],
                body: FnBody::Native(std::sync::Arc::new(|args| {
                    let s = args
                        .iter()
                        .map(|v| v.to_string())
                        .collect::<Vec<_>>()
                        .join(" ");
                    print!("{s}");
                    Ok(Value::Unit)
                })),
                closure: Env::default(),
            }),
        );
        // int()
        self.define(
            "int",
            Value::Fn(FnValue {
                name: Some("int".into()),
                params: vec![],
                body: FnBody::Native(std::sync::Arc::new(|args| match args.first() {
                    Some(Value::Int(n)) => Ok(Value::Int(*n)),
                    Some(Value::Float(f)) => Ok(Value::Int(*f as i64)),
                    Some(Value::Str(s)) => s
                        .parse::<i64>()
                        .map(Value::Int)
                        .map_err(|_| InterpError::runtime(format!("cannot convert '{s}' to int"))),
                    Some(Value::Bool(b)) => Ok(Value::Int(if *b { 1 } else { 0 })),
                    _ => Err(InterpError::runtime("int() requires one argument")),
                })),
                closure: Env::default(),
            }),
        );
        // float()
        self.define(
            "float",
            Value::Fn(FnValue {
                name: Some("float".into()),
                params: vec![],
                body: FnBody::Native(std::sync::Arc::new(|args| match args.first() {
                    Some(Value::Int(n)) => Ok(Value::Float(*n as f64)),
                    Some(Value::Float(f)) => Ok(Value::Float(*f)),
                    Some(Value::Str(s)) => s.parse::<f64>().map(Value::Float).map_err(|_| {
                        InterpError::runtime(format!("cannot convert '{s}' to float"))
                    }),
                    _ => Err(InterpError::runtime("float() requires one argument")),
                })),
                closure: Env::default(),
            }),
        );
        // str()
        self.define(
            "str",
            Value::Fn(FnValue {
                name: Some("str".into()),
                params: vec![],
                body: FnBody::Native(std::sync::Arc::new(|args| {
                    Ok(Value::Str(
                        args.first().map(|v| v.to_string()).unwrap_or_default(),
                    ))
                })),
                closure: Env::default(),
            }),
        );
        // bool()
        self.define(
            "bool",
            Value::Fn(FnValue {
                name: Some("bool".into()),
                params: vec![],
                body: FnBody::Native(std::sync::Arc::new(|args| match args.first() {
                    Some(Value::Bool(b)) => Ok(Value::Bool(*b)),
                    Some(Value::Int(n)) => Ok(Value::Bool(*n != 0)),
                    Some(Value::Str(s)) => Ok(Value::Bool(!s.is_empty())),
                    Some(Value::Unit) => Ok(Value::Bool(false)),
                    None => Ok(Value::Bool(false)),
                    _ => Ok(Value::Bool(true)),
                })),
                closure: Env::default(),
            }),
        );
        // abs, min, max
        self.define(
            "abs",
            Value::Fn(FnValue {
                name: Some("abs".into()),
                params: vec![],
                body: FnBody::Native(std::sync::Arc::new(|args| match args.first() {
                    Some(Value::Int(n)) => Ok(Value::Int(n.abs())),
                    Some(Value::Float(f)) => Ok(Value::Float(f.abs())),
                    _ => Err(InterpError::runtime("abs() requires a number")),
                })),
                closure: Env::default(),
            }),
        );
        self.define(
            "min",
            Value::Fn(FnValue {
                name: Some("min".into()),
                params: vec![],
                body: FnBody::Native(std::sync::Arc::new(|args| {
                    if args.len() != 2 {
                        return Err(InterpError::runtime("min() requires 2 args"));
                    }
                    match (&args[0], &args[1]) {
                        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(*a.min(b))),
                        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a.min(*b))),
                        _ => Err(InterpError::runtime("min() requires numbers")),
                    }
                })),
                closure: Env::default(),
            }),
        );
        self.define(
            "max",
            Value::Fn(FnValue {
                name: Some("max".into()),
                params: vec![],
                body: FnBody::Native(std::sync::Arc::new(|args| {
                    if args.len() != 2 {
                        return Err(InterpError::runtime("max() requires 2 args"));
                    }
                    match (&args[0], &args[1]) {
                        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(*a.max(b))),
                        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a.max(*b))),
                        _ => Err(InterpError::runtime("max() requires numbers")),
                    }
                })),
                closure: Env::default(),
            }),
        );
        // fmt
        self.define(
            "fmt",
            Value::Fn(FnValue {
                name: Some("fmt".into()),
                params: vec![],
                body: FnBody::Native(std::sync::Arc::new(|args| {
                    if args.is_empty() {
                        return Err(InterpError::runtime("fmt() requires at least 1 arg"));
                    }
                    let template = match &args[0] {
                        Value::Str(s) => s.clone(),
                        _ => return Err(InterpError::runtime("fmt() first arg must be a string")),
                    };
                    let mut result = template.clone();
                    for arg in &args[1..] {
                        result = result.replacen("{}", &arg.to_string(), 1);
                    }
                    Ok(Value::Str(result))
                })),
                closure: Env::default(),
            }),
        );
        // filter(list, fn) — prefix form
        self.define(
            "filter",
            Value::Fn(FnValue {
                name: Some("filter".into()),
                params: vec![],
                body: FnBody::Native(std::sync::Arc::new(|_args| {
                    // Will be called through the interpreter's call mechanism
                    Err(InterpError::runtime(
                        "filter() must be called through interpreter",
                    ))
                })),
                closure: Env::default(),
            }),
        );
        // map(list, fn)
        self.define(
            "map",
            Value::Fn(FnValue {
                name: Some("map".into()),
                params: vec![],
                body: FnBody::Native(std::sync::Arc::new(|_args| {
                    Err(InterpError::runtime(
                        "map() must be called through interpreter",
                    ))
                })),
                closure: Env::default(),
            }),
        );

        // -- reduce(list, fn, init) ------------------------------------------
        self.define(
            "reduce",
            Value::Fn(FnValue {
                name: Some("reduce".into()),
                params: vec![],
                body: FnBody::Native(std::sync::Arc::new(|_args| {
                    Err(InterpError::runtime(
                        "reduce() must be called through interpreter",
                    ))
                })),
                closure: Env::default(),
            }),
        );

        // -- zip(a b) --------------------------------------------------------
        self.define(
            "zip",
            Value::Fn(FnValue {
                name: Some("zip".into()),
                params: vec![],
                body: FnBody::Native(std::sync::Arc::new(|args| {
                    if args.len() < 2 {
                        return Err(InterpError::runtime("zip() requires 2 lists"));
                    }
                    match (&args[0], &args[1]) {
                        (Value::List(a), Value::List(b)) => Ok(Value::List(
                            a.iter()
                                .zip(b.iter())
                                .map(|(x, y)| Value::Tuple(vec![x.clone(), y.clone()]))
                                .collect(),
                        )),
                        _ => Err(InterpError::runtime("zip() requires two lists")),
                    }
                })),
                closure: Env::default(),
            }),
        );

        // -- flat(list) ------------------------------------------------------
        self.define(
            "flat",
            Value::Fn(FnValue {
                name: Some("flat".into()),
                params: vec![],
                body: FnBody::Native(std::sync::Arc::new(|args| match args.first() {
                    Some(Value::List(items)) => {
                        let mut result = vec![];
                        for item in items {
                            match item {
                                Value::List(inner) => result.extend(inner.clone()),
                                other => result.push(other.clone()),
                            }
                        }
                        Ok(Value::List(result))
                    }
                    _ => Err(InterpError::runtime("flat() requires a list")),
                })),
                closure: Env::default(),
            }),
        );

        // -- any(list, fn) ---------------------------------------------------
        self.define(
            "any",
            Value::Fn(FnValue {
                name: Some("any".into()),
                params: vec![],
                body: FnBody::Native(std::sync::Arc::new(|_args| {
                    Err(InterpError::runtime(
                        "any() must be called through interpreter",
                    ))
                })),
                closure: Env::default(),
            }),
        );

        // -- all(list, fn) ---------------------------------------------------
        self.define(
            "all",
            Value::Fn(FnValue {
                name: Some("all".into()),
                params: vec![],
                body: FnBody::Native(std::sync::Arc::new(|_args| {
                    Err(InterpError::runtime(
                        "all() must be called through interpreter",
                    ))
                })),
                closure: Env::default(),
            }),
        );

        // -- clamp(x lo hi) --------------------------------------------------
        self.define(
            "clamp",
            Value::Fn(FnValue {
                name: Some("clamp".into()),
                params: vec![],
                body: FnBody::Native(std::sync::Arc::new(|args| {
                    if args.len() < 3 {
                        return Err(InterpError::runtime("clamp() requires 3 args"));
                    }
                    match (&args[0], &args[1], &args[2]) {
                        (Value::Int(x), Value::Int(lo), Value::Int(hi)) => {
                            Ok(Value::Int((*x).clamp(*lo, *hi)))
                        }
                        (Value::Float(x), Value::Float(lo), Value::Float(hi)) => {
                            Ok(Value::Float(x.clamp(*lo, *hi)))
                        }
                        _ => Err(InterpError::runtime("clamp() requires numbers")),
                    }
                })),
                closure: Env::default(),
            }),
        );

        // -- math.* ----------------------------------------------------------
        macro_rules! math_fn1 {
            ($name:expr, $op:expr) => {
                self.define(
                    $name,
                    Value::Fn(FnValue {
                        name: Some($name.into()),
                        params: vec![],
                        body: FnBody::Native(std::sync::Arc::new(|args| match args.first() {
                            Some(Value::Float(f)) => Ok(Value::Float($op(*f))),
                            Some(Value::Int(n)) => Ok(Value::Float($op(*n as f64))),
                            _ => Err(InterpError::runtime(concat!($name, "() requires a number"))),
                        })),
                        closure: Env::default(),
                    }),
                );
            };
        }
        // Register math namespace as "math.fn" names
        self.define(
            "math.floor",
            Value::Fn(FnValue {
                name: Some("math.floor".into()),
                params: vec![],
                body: FnBody::Native(std::sync::Arc::new(|args| match args.first() {
                    Some(Value::Float(f)) => Ok(Value::Float(f.floor())),
                    Some(Value::Int(n)) => Ok(Value::Float((*n as f64).floor())),
                    _ => Err(InterpError::runtime("math.floor requires number")),
                })),
                closure: Env::default(),
            }),
        );
        self.define(
            "math.ceil",
            Value::Fn(FnValue {
                name: Some("math.ceil".into()),
                params: vec![],
                body: FnBody::Native(std::sync::Arc::new(|args| match args.first() {
                    Some(Value::Float(f)) => Ok(Value::Float(f.ceil())),
                    Some(Value::Int(n)) => Ok(Value::Float((*n as f64).ceil())),
                    _ => Err(InterpError::runtime("math.ceil requires number")),
                })),
                closure: Env::default(),
            }),
        );
        self.define(
            "math.round",
            Value::Fn(FnValue {
                name: Some("math.round".into()),
                params: vec![],
                body: FnBody::Native(std::sync::Arc::new(|args| match args.first() {
                    Some(Value::Float(f)) => Ok(Value::Float(f.round())),
                    Some(Value::Int(n)) => Ok(Value::Float((*n as f64).round())),
                    _ => Err(InterpError::runtime("math.round requires number")),
                })),
                closure: Env::default(),
            }),
        );
        self.define(
            "math.sqrt",
            Value::Fn(FnValue {
                name: Some("math.sqrt".into()),
                params: vec![],
                body: FnBody::Native(std::sync::Arc::new(|args| match args.first() {
                    Some(Value::Float(f)) => Ok(Value::Float(f.sqrt())),
                    Some(Value::Int(n)) => Ok(Value::Float((*n as f64).sqrt())),
                    _ => Err(InterpError::runtime("math.sqrt requires number")),
                })),
                closure: Env::default(),
            }),
        );
        self.define(
            "math.sin",
            Value::Fn(FnValue {
                name: Some("math.sin".into()),
                params: vec![],
                body: FnBody::Native(std::sync::Arc::new(|args| match args.first() {
                    Some(Value::Float(f)) => Ok(Value::Float(f.sin())),
                    Some(Value::Int(n)) => Ok(Value::Float((*n as f64).sin())),
                    _ => Err(InterpError::runtime("math.sin requires number")),
                })),
                closure: Env::default(),
            }),
        );
        self.define(
            "math.cos",
            Value::Fn(FnValue {
                name: Some("math.cos".into()),
                params: vec![],
                body: FnBody::Native(std::sync::Arc::new(|args| match args.first() {
                    Some(Value::Float(f)) => Ok(Value::Float(f.cos())),
                    Some(Value::Int(n)) => Ok(Value::Float((*n as f64).cos())),
                    _ => Err(InterpError::runtime("math.cos requires number")),
                })),
                closure: Env::default(),
            }),
        );
        self.define(
            "math.tan",
            Value::Fn(FnValue {
                name: Some("math.tan".into()),
                params: vec![],
                body: FnBody::Native(std::sync::Arc::new(|args| match args.first() {
                    Some(Value::Float(f)) => Ok(Value::Float(f.tan())),
                    Some(Value::Int(n)) => Ok(Value::Float((*n as f64).tan())),
                    _ => Err(InterpError::runtime("math.tan requires number")),
                })),
                closure: Env::default(),
            }),
        );
        self.define(
            "math.log",
            Value::Fn(FnValue {
                name: Some("math.log".into()),
                params: vec![],
                body: FnBody::Native(std::sync::Arc::new(|args| match args.first() {
                    Some(Value::Float(f)) => Ok(Value::Float(f.ln())),
                    Some(Value::Int(n)) => Ok(Value::Float((*n as f64).ln())),
                    _ => Err(InterpError::runtime("math.log requires number")),
                })),
                closure: Env::default(),
            }),
        );
        self.define(
            "math.log2",
            Value::Fn(FnValue {
                name: Some("math.log2".into()),
                params: vec![],
                body: FnBody::Native(std::sync::Arc::new(|args| match args.first() {
                    Some(Value::Float(f)) => Ok(Value::Float(f.log2())),
                    Some(Value::Int(n)) => Ok(Value::Float((*n as f64).log2())),
                    _ => Err(InterpError::runtime("math.log2 requires number")),
                })),
                closure: Env::default(),
            }),
        );
        self.define(
            "math.log10",
            Value::Fn(FnValue {
                name: Some("math.log10".into()),
                params: vec![],
                body: FnBody::Native(std::sync::Arc::new(|args| match args.first() {
                    Some(Value::Float(f)) => Ok(Value::Float(f.log10())),
                    Some(Value::Int(n)) => Ok(Value::Float((*n as f64).log10())),
                    _ => Err(InterpError::runtime("math.log10 requires number")),
                })),
                closure: Env::default(),
            }),
        );
        self.define("math.pi", Value::Float(std::f64::consts::PI));
        self.define("math.e", Value::Float(std::f64::consts::E));
        self.define(
            "math.pow",
            Value::Fn(FnValue {
                name: Some("math.pow".into()),
                params: vec![],
                body: FnBody::Native(std::sync::Arc::new(|args| {
                    if args.len() < 2 {
                        return Err(InterpError::runtime("math.pow requires 2 args"));
                    }
                    match (&args[0], &args[1]) {
                        (Value::Float(x), Value::Float(e)) => Ok(Value::Float(x.powf(*e))),
                        (Value::Int(x), Value::Int(e)) => {
                            Ok(Value::Float((*x as f64).powf(*e as f64)))
                        }
                        _ => Err(InterpError::runtime("math.pow requires numbers")),
                    }
                })),
                closure: Env::default(),
            }),
        );

        // -- env.* -----------------------------------------------------------
        self.define(
            "env.get",
            Value::Fn(FnValue {
                name: Some("env.get".into()),
                params: vec![],
                body: FnBody::Native(std::sync::Arc::new(|args| {
                    let key = match args.first() {
                        Some(Value::Str(s)) => s.clone(),
                        _ => return Err(InterpError::runtime("env.get requires string key")),
                    };
                    Ok(std::env::var(&key)
                        .ok()
                        .map(|v| Value::Option(Some(Box::new(Value::Str(v)))))
                        .unwrap_or(Value::Option(None)))
                })),
                closure: Env::default(),
            }),
        );
        self.define(
            "env.require",
            Value::Fn(FnValue {
                name: Some("env.require".into()),
                params: vec![],
                body: FnBody::Native(std::sync::Arc::new(|args| {
                    let key = match args.first() {
                        Some(Value::Str(s)) => s.clone(),
                        _ => return Err(InterpError::runtime("env.require requires string key")),
                    };
                    std::env::var(&key).map(Value::Str).map_err(|_| {
                        InterpError::runtime(format!("required env var '{key}' not set"))
                    })
                })),
                closure: Env::default(),
            }),
        );

        // -- file.* ----------------------------------------------------------
        self.define(
            "file.read",
            Value::Fn(FnValue {
                name: Some("file.read".into()),
                params: vec![],
                body: FnBody::Native(std::sync::Arc::new(|args| {
                    let path = match args.first() {
                        Some(Value::Str(s)) => s.clone(),
                        _ => return Err(InterpError::runtime("file.read requires path")),
                    };
                    Ok(std::fs::read_to_string(&path)
                        .ok()
                        .map(|s| Value::Option(Some(Box::new(Value::Str(s)))))
                        .unwrap_or(Value::Option(None)))
                })),
                closure: Env::default(),
            }),
        );
        self.define(
            "file.write",
            Value::Fn(FnValue {
                name: Some("file.write".into()),
                params: vec![],
                body: FnBody::Native(std::sync::Arc::new(|args| {
                    if args.len() < 2 {
                        return Err(InterpError::runtime("file.write requires path and data"));
                    }
                    let (path, data) = match (&args[0], &args[1]) {
                        (Value::Str(p), Value::Str(d)) => (p.clone(), d.clone()),
                        _ => return Err(InterpError::runtime("file.write requires strings")),
                    };
                    std::fs::write(&path, &data)
                        .map(|_| Value::Unit)
                        .map_err(|e| InterpError::runtime(e.to_string()))
                })),
                closure: Env::default(),
            }),
        );
        self.define(
            "file.exists",
            Value::Fn(FnValue {
                name: Some("file.exists".into()),
                params: vec![],
                body: FnBody::Native(std::sync::Arc::new(|args| {
                    let path = match args.first() {
                        Some(Value::Str(s)) => s.clone(),
                        _ => return Err(InterpError::runtime("file.exists requires path")),
                    };
                    Ok(Value::Bool(std::path::Path::new(&path).exists()))
                })),
                closure: Env::default(),
            }),
        );
        self.define(
            "file.rm",
            Value::Fn(FnValue {
                name: Some("file.rm".into()),
                params: vec![],
                body: FnBody::Native(std::sync::Arc::new(|args| {
                    let path = match args.first() {
                        Some(Value::Str(s)) => s.clone(),
                        _ => return Err(InterpError::runtime("file.rm requires path")),
                    };
                    let p = std::path::Path::new(&path);
                    let r = if p.is_dir() {
                        std::fs::remove_dir_all(p)
                    } else {
                        std::fs::remove_file(p)
                    };
                    r.map(|_| Value::Unit)
                        .map_err(|e| InterpError::runtime(e.to_string()))
                })),
                closure: Env::default(),
            }),
        );
        self.define(
            "file.ls",
            Value::Fn(FnValue {
                name: Some("file.ls".into()),
                params: vec![],
                body: FnBody::Native(std::sync::Arc::new(|args| {
                    let dir = match args.first() {
                        Some(Value::Str(s)) => s.clone(),
                        _ => return Err(InterpError::runtime("file.ls requires path")),
                    };
                    let entries =
                        std::fs::read_dir(&dir).map_err(|e| InterpError::runtime(e.to_string()))?;
                    Ok(Value::List(
                        entries
                            .filter_map(|e| e.ok())
                            .map(|e| Value::Str(e.file_name().to_string_lossy().to_string()))
                            .collect(),
                    ))
                })),
                closure: Env::default(),
            }),
        );
        self.define(
            "file.mkdir",
            Value::Fn(FnValue {
                name: Some("file.mkdir".into()),
                params: vec![],
                body: FnBody::Native(std::sync::Arc::new(|args| {
                    let path = match args.first() {
                        Some(Value::Str(s)) => s.clone(),
                        _ => return Err(InterpError::runtime("file.mkdir requires path")),
                    };
                    std::fs::create_dir_all(&path)
                        .map(|_| Value::Unit)
                        .map_err(|e| InterpError::runtime(e.to_string()))
                })),
                closure: Env::default(),
            }),
        );

        // -- json.* ----------------------------------------------------------
        self.define(
            "json.str",
            Value::Fn(FnValue {
                name: Some("json.str".into()),
                params: vec![],
                body: FnBody::Native(std::sync::Arc::new(|args| {
                    let s = args.first().map(|v| v.to_string()).unwrap_or_default();
                    Ok(Value::Str(s))
                })),
                closure: Env::default(),
            }),
        );

        // -- Built-in Option/Result constructors -----------------------------
        self.define("none", Value::Option(None));
        self.define("None", Value::Option(None));
        self.define(
            "Some",
            Value::Fn(FnValue {
                name: Some("Some".into()),
                params: vec![],
                body: FnBody::Native(std::sync::Arc::new(|args| {
                    let v = args.first().cloned().unwrap_or(Value::Unit);
                    Ok(Value::Option(Some(Box::new(v))))
                })),
                closure: Env::default(),
            }),
        );
        self.define(
            "Ok",
            Value::Fn(FnValue {
                name: Some("Ok".into()),
                params: vec![],
                body: FnBody::Native(std::sync::Arc::new(|args| {
                    let v = args.first().cloned().unwrap_or(Value::Unit);
                    Ok(Value::Ok(Box::new(v)))
                })),
                closure: Env::default(),
            }),
        );
        self.define(
            "Err",
            Value::Fn(FnValue {
                name: Some("Err".into()),
                params: vec![],
                body: FnBody::Native(std::sync::Arc::new(|args| {
                    let v = args.first().cloned().unwrap_or(Value::Unit);
                    Ok(Value::Err(Box::new(v)))
                })),
                closure: Env::default(),
            }),
        );

        // -- env.set ---------------------------------------------------------
        self.define(
            "env.set",
            Value::Fn(FnValue {
                name: Some("env.set".into()),
                params: vec![],
                body: FnBody::Native(std::sync::Arc::new(|args| {
                    if args.len() < 2 {
                        return Err(InterpError::runtime("env.set requires key and value"));
                    }
                    match (&args[0], &args[1]) {
                        (Value::Str(k), Value::Str(v)) => {
                            std::env::set_var(k, v);
                            Ok(Value::Unit)
                        }
                        _ => Err(InterpError::runtime(
                            "env.set requires string key and value",
                        )),
                    }
                })),
                closure: Env::default(),
            }),
        );

        // -- file.append -----------------------------------------------------
        self.define(
            "file.append",
            Value::Fn(FnValue {
                name: Some("file.append".into()),
                params: vec![],
                body: FnBody::Native(std::sync::Arc::new(|args| {
                    use std::io::Write;
                    if args.len() < 2 {
                        return Err(InterpError::runtime("file.append requires path and data"));
                    }
                    match (&args[0], &args[1]) {
                        (Value::Str(path), Value::Str(data)) => {
                            let mut f = std::fs::OpenOptions::new()
                                .append(true)
                                .create(true)
                                .open(path)
                                .map_err(|e| InterpError::runtime(e.to_string()))?;
                            f.write_all(data.as_bytes())
                                .map_err(|e| InterpError::runtime(e.to_string()))?;
                            Ok(Value::Unit)
                        }
                        _ => Err(InterpError::runtime(
                            "file.append requires string path and data",
                        )),
                    }
                })),
                closure: Env::default(),
            }),
        );

        // -- math.clamp ------------------------------------------------------
        self.define(
            "math.clamp",
            Value::Fn(FnValue {
                name: Some("math.clamp".into()),
                params: vec![],
                body: FnBody::Native(std::sync::Arc::new(|args| {
                    if args.len() < 3 {
                        return Err(InterpError::runtime("math.clamp requires 3 args"));
                    }
                    match (&args[0], &args[1], &args[2]) {
                        (Value::Float(x), Value::Float(lo), Value::Float(hi)) => {
                            Ok(Value::Float(x.clamp(*lo, *hi)))
                        }
                        (Value::Int(x), Value::Int(lo), Value::Int(hi)) => {
                            Ok(Value::Int((*x).clamp(*lo, *hi)))
                        }
                        _ => Err(InterpError::runtime("math.clamp requires numbers")),
                    }
                })),
                closure: Env::default(),
            }),
        );
    }
}

// --- Error --------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct InterpError {
    pub kind: ErrorKind,
    pub msg: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ErrorKind {
    Runtime,
    Panic,
    // Used internally to unwind the call stack
    Return,
    Propagated, // ! operator on Err value
}

impl InterpError {
    pub fn runtime(msg: impl Into<String>) -> Self {
        InterpError {
            kind: ErrorKind::Runtime,
            msg: msg.into(),
        }
    }
    pub fn panic(msg: impl Into<String>) -> Self {
        InterpError {
            kind: ErrorKind::Panic,
            msg: msg.into(),
        }
    }
    fn return_val(msg: impl Into<String>) -> Self {
        InterpError {
            kind: ErrorKind::Return,
            msg: msg.into(),
        }
    }
}

impl fmt::Display for InterpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            ErrorKind::Panic => write!(f, "panic: {}", self.msg),
            ErrorKind::Runtime => write!(f, "runtime error: {}", self.msg),
            ErrorKind::Return => write!(f, "<return: {}>", self.msg),
            ErrorKind::Propagated => write!(f, "propagated error: {}", self.msg),
        }
    }
}

pub type InterpResult<T> = Result<T, InterpError>;

// --- Interpreter -------------------------------------------------------------

pub struct Interpreter {
    env: Env,
    // Return value channel — used to unwind from return statements
    return_value: Option<Value>,
}

impl Interpreter {
    pub fn new() -> Self {
        Interpreter {
            env: Env::new(),
            return_value: None,
        }
    }

    pub fn run(&mut self, program: &Program) -> InterpResult<Value> {
        let mut last = Value::Unit;
        for stmt in &program.stmts {
            last = self.exec_stmt(stmt)?;
        }
        Ok(last)
    }

    /// Run a program reusing this interpreter's environment (for REPL use).
    pub fn run_program(&mut self, program: &Program) -> InterpResult<Value> {
        self.run(program)
    }

    // -- statements -----------------------------------------------------------

    fn exec_stmt(&mut self, stmt: &Stmt) -> InterpResult<Value> {
        match &stmt.kind {
            StmtKind::Let { name, value, .. } => {
                let v = self.eval_expr(value)?;
                self.env.define(name, v);
                Ok(Value::Unit)
            }
            StmtKind::Assign { target, value } => {
                let v = self.eval_expr(value)?;
                self.assign_target(target, v)?;
                Ok(Value::Unit)
            }
            StmtKind::Return(expr) => {
                let v = match expr {
                    Some(e) => self.eval_expr(e)?,
                    None => Value::Unit,
                };
                self.return_value = Some(v.clone());
                Err(InterpError {
                    kind: ErrorKind::Return,
                    msg: String::new(),
                })
            }
            StmtKind::Panic(expr) => {
                let v = self.eval_expr(expr)?;
                Err(InterpError::panic(v.to_string()))
            }
            StmtKind::While { cond, body } => {
                loop {
                    let c = self.eval_expr(cond)?;
                    if !self.is_truthy(&c) {
                        break;
                    }
                    match self.exec_block(body) {
                        Ok(_) => {}
                        Err(e) if e.kind == ErrorKind::Return => return Err(e),
                        Err(e) => return Err(e),
                    }
                }
                Ok(Value::Unit)
            }
            StmtKind::For { var, iter, body } => {
                let iterable = self.eval_expr(iter)?;
                let items = self.to_iter(iterable)?;
                for item in items {
                    self.env.push();
                    self.env.define(var, item);
                    match self.exec_block(body) {
                        Ok(_) => {}
                        Err(e) if e.kind == ErrorKind::Return => {
                            self.env.pop();
                            return Err(e);
                        }
                        Err(e) => {
                            self.env.pop();
                            return Err(e);
                        }
                    }
                    self.env.pop();
                }
                Ok(Value::Unit)
            }
            StmtKind::FnDef(fndef) => {
                // Build initial fn value with current env as closure
                let mut closure = self.env.clone();
                // Define a placeholder first so recursive calls can find the name
                // We'll overwrite it with the real value right after
                let placeholder = FnValue {
                    name: Some(fndef.name.clone()),
                    params: fndef.params.clone(),
                    body: FnBody::Ast(fndef.body.stmts.clone()),
                    closure: closure.clone(),
                };
                // Define in current env (so the closure below can reference it)
                self.env.define(&fndef.name, Value::Fn(placeholder));
                // Now build the real fn with an updated closure that includes itself
                closure.define(&fndef.name, self.env.get(&fndef.name).unwrap().clone());
                let real_fn = FnValue {
                    name: Some(fndef.name.clone()),
                    params: fndef.params.clone(),
                    body: FnBody::Ast(fndef.body.stmts.clone()),
                    closure,
                };
                self.env.define(&fndef.name, Value::Fn(real_fn));
                Ok(Value::Unit)
            }
            StmtKind::TypeDef(td) => {
                // Register constructor functions for union variants
                if let TypeDefKind::Union(variants) = &td.kind {
                    for variant in variants {
                        let vname = variant.name.clone();
                        let arity = variant.fields.len();
                        let vname2 = vname.clone();
                        self.env.define(
                            &vname2,
                            Value::Fn(FnValue {
                                name: Some(vname.clone()),
                                params: vec![],
                                body: FnBody::Native(std::sync::Arc::new(move |args| {
                                    if args.len() != arity {
                                        return Err(InterpError::runtime(format!(
                                            "variant {vname} expects {arity} args, got {}",
                                            args.len()
                                        )));
                                    }
                                    Ok(Value::Variant(vname.clone(), args.to_vec()))
                                })),
                                closure: Env::default(),
                            }),
                        );
                    }
                }
                Ok(Value::Unit)
            }
            StmtKind::Expr(expr) => self.eval_expr(expr),
        }
    }

    fn exec_block(&mut self, block: &Block) -> InterpResult<Value> {
        self.env.push();
        let mut last = Value::Unit;
        for stmt in &block.stmts {
            match self.exec_stmt(stmt) {
                Ok(v) => last = v,
                Err(e) => {
                    self.env.pop();
                    return Err(e);
                }
            }
        }
        self.env.pop();
        Ok(last)
    }

    fn assign_target(&mut self, target: &Expr, val: Value) -> InterpResult<()> {
        match &target.kind {
            ExprKind::Ident(name) => {
                self.env.set(name, val);
                Ok(())
            }
            // list[i] = val  — mutate list in place
            ExprKind::Index { obj, index } => {
                if let ExprKind::Ident(list_name) = &obj.kind {
                    let idx_val = self.eval_expr(index)?;
                    let idx = match &idx_val {
                        Value::Int(n) => *n as usize,
                        _ => return Err(InterpError::runtime("list index must be an integer")),
                    };
                    let list = self.env.get(list_name).ok_or_else(|| {
                        InterpError::runtime(format!("undefined variable '{list_name}'"))
                    })?;
                    match list {
                        Value::List(mut items) => {
                            if idx >= items.len() {
                                return Err(InterpError::runtime(format!(
                                    "index {idx} out of bounds (len {})",
                                    items.len()
                                )));
                            }
                            items[idx] = val;
                            self.env.set(list_name, Value::List(items));
                            Ok(())
                        }
                        _ => Err(InterpError::runtime(format!("'{list_name}' is not a list"))),
                    }
                } else {
                    Err(InterpError::runtime(
                        "index assignment only supported on named lists",
                    ))
                }
            }
            // obj.field = val  — mutate struct field
            ExprKind::Field { obj, field } => {
                if let ExprKind::Ident(obj_name) = &obj.kind {
                    let obj_val = self.env.get(obj_name).ok_or_else(|| {
                        InterpError::runtime(format!("undefined variable '{obj_name}'"))
                    })?;
                    match obj_val {
                        Value::Struct(type_name, mut fields) => {
                            fields.insert(field.clone(), val);
                            self.env.set(obj_name, Value::Struct(type_name, fields));
                            Ok(())
                        }
                        _ => Err(InterpError::runtime(format!(
                            "field assignment on non-struct '{obj_name}'"
                        ))),
                    }
                } else {
                    Err(InterpError::runtime(
                        "field assignment only supported on named variables",
                    ))
                }
            }
            _ => Err(InterpError::runtime("invalid assignment target")),
        }
    }

    // -- expressions ----------------------------------------------------------

    fn eval_expr(&mut self, expr: &Expr) -> InterpResult<Value> {
        match &expr.kind {
            ExprKind::Int(n) => Ok(Value::Int(*n)),
            ExprKind::Float(n) => Ok(Value::Float(*n)),
            ExprKind::Bool(b) => Ok(Value::Bool(*b)),
            ExprKind::Str(s) => Ok(Value::Str(self.interpolate(s)?)),

            ExprKind::Ident(name) => self
                .env
                .get(name)
                .ok_or_else(|| InterpError::runtime(format!("undefined variable '{name}'"))),

            ExprKind::List(items) => {
                let vs: InterpResult<Vec<_>> = items.iter().map(|e| self.eval_expr(e)).collect();
                Ok(Value::List(vs?))
            }

            ExprKind::Tuple(items) => {
                let vs: InterpResult<Vec<_>> = items.iter().map(|e| self.eval_expr(e)).collect();
                Ok(Value::Tuple(vs?))
            }

            ExprKind::Map(pairs) => {
                let mut result = vec![];
                for (k, v) in pairs {
                    result.push((self.eval_expr(k)?, self.eval_expr(v)?));
                }
                Ok(Value::Map(result))
            }

            ExprKind::BinOp { op, lhs, rhs } => self.eval_binop(op, lhs, rhs),

            ExprKind::UnOp { op, expr } => {
                let v = self.eval_expr(expr)?;
                match op {
                    UnOp::Neg => match v {
                        Value::Int(n) => Ok(Value::Int(-n)),
                        Value::Float(f) => Ok(Value::Float(-f)),
                        _ => Err(InterpError::runtime("negation requires a number")),
                    },
                    UnOp::Not => Ok(Value::Bool(!self.is_truthy(&v))),
                }
            }

            ExprKind::Call { callee, args } => {
                // Method call: obj.method(args) — pass receiver as implicit first arg
                if let ExprKind::Field { obj, field } = &callee.kind {
                    // Check if this is a namespace call: math.sqrt, file.read, etc.
                    if let ExprKind::Ident(ns) = &obj.kind {
                        let full_name = format!("{ns}.{field}");
                        if let Some(fn_val) = self.env.get(&full_name) {
                            let arg_vals: Vec<Value> = args
                                .iter()
                                .map(|a| self.eval_expr(a))
                                .collect::<InterpResult<_>>()?;
                            return self.call_fn(fn_val, arg_vals);
                        }
                    }
                    let receiver = self.eval_expr(obj)?;
                    let mut arg_vals: Vec<Value> = args
                        .iter()
                        .map(|a| self.eval_expr(a))
                        .collect::<InterpResult<_>>()?;
                    return self.call_method(receiver, field, &mut arg_vals);
                }
                let fn_val = self.eval_expr(callee)?;
                let arg_vals: InterpResult<Vec<_>> =
                    args.iter().map(|a| self.eval_expr(a)).collect();
                self.call_fn(fn_val, arg_vals?)
            }

            ExprKind::Field { obj, field } => {
                // Namespace constant lookup: math.pi, math.e
                if let ExprKind::Ident(ns) = &obj.kind {
                    let full_name = format!("{ns}.{field}");
                    if let Some(val) = self.env.get(&full_name) {
                        return Ok(val);
                    }
                }
                let v = self.eval_expr(obj)?;
                self.eval_field(v, field)
            }

            ExprKind::SafeField { obj, field } => {
                let v = self.eval_expr(obj)?;
                match v {
                    Value::Option(None) => Ok(Value::Option(None)),
                    Value::Option(Some(inner)) => Ok(Value::Option(Some(Box::new(
                        self.eval_field(*inner, field)?,
                    )))),
                    other => self.eval_field(other, field),
                }
            }

            ExprKind::Index { obj, index } => {
                let v = self.eval_expr(obj)?;
                let i = self.eval_expr(index)?;
                match (v, i) {
                    (Value::List(items), Value::Int(idx)) => {
                        let idx = if idx < 0 {
                            items.len() as i64 + idx
                        } else {
                            idx
                        } as usize;
                        items
                            .get(idx)
                            .cloned()
                            .map(|v| Value::Option(Some(Box::new(v))))
                            .ok_or_else(|| {
                                InterpError::runtime(format!("index {idx} out of bounds"))
                            })
                    }
                    (Value::Map(pairs), key) => Ok(pairs
                        .into_iter()
                        .find(|(k, _)| *k == key)
                        .map(|(_, v)| Value::Option(Some(Box::new(v))))
                        .unwrap_or(Value::Option(None))),
                    _ => Err(InterpError::runtime("invalid index operation")),
                }
            }

            ExprKind::Pipe { lhs, rhs } => {
                let left = self.eval_expr(lhs)?;
                // a |> f(b c)  =>  f(a b c)  — lhs inserted as first argument
                if let ExprKind::Call { callee, args } = &rhs.kind {
                    // Method call on pipe: a |> list.method(args) - treat as method
                    if let ExprKind::Field { obj, field } = &callee.kind {
                        let receiver = self.eval_expr(obj)?;
                        let mut arg_vals: Vec<Value> = args
                            .iter()
                            .map(|a| self.eval_expr(a))
                            .collect::<InterpResult<_>>()?;
                        arg_vals.insert(0, left);
                        return self.call_method(receiver, field, &mut arg_vals);
                    }
                    // Regular call: insert lhs as first arg
                    let func = self.eval_expr(callee)?;
                    let mut arg_vals: Vec<Value> = args
                        .iter()
                        .map(|a| self.eval_expr(a))
                        .collect::<InterpResult<_>>()?;
                    arg_vals.insert(0, left);
                    return self.call_fn(func, arg_vals);
                }
                // a |> f  =>  f(a)
                let func = self.eval_expr(rhs)?;
                self.call_fn(func, vec![left])
            }

            ExprKind::NullCoalesce { lhs, rhs } => {
                let left = self.eval_expr(lhs)?;
                match left {
                    Value::Option(None) => self.eval_expr(rhs),
                    Value::Option(Some(v)) => Ok(*v),
                    other => Ok(other),
                }
            }

            ExprKind::Propagate(expr) => {
                let v = self.eval_expr(expr)?;
                match v {
                    Value::Err(e) => Err(InterpError {
                        kind: ErrorKind::Propagated,
                        msg: e.to_string(),
                    }),
                    Value::Ok(v) => Ok(*v),
                    // User-defined Err-named variant
                    Value::Variant(ref name, ref fields) if name == "Err" => {
                        let msg = fields.first().map(|v| v.to_string()).unwrap_or_default();
                        Err(InterpError {
                            kind: ErrorKind::Propagated,
                            msg,
                        })
                    }
                    // Option None propagation (? operator semantics)
                    Value::Option(None) => Err(InterpError {
                        kind: ErrorKind::Propagated,
                        msg: "none".into(),
                    }),
                    Value::Option(Some(v)) => Ok(*v),
                    other => Ok(other),
                }
            }

            ExprKind::Range { start, end } => {
                let s = self.eval_expr(start)?;
                let e = self.eval_expr(end)?;
                match (s, e) {
                    (Value::Int(a), Value::Int(b)) => {
                        Ok(Value::List((a..b).map(Value::Int).collect()))
                    }
                    _ => Err(InterpError::runtime("range requires integers")),
                }
            }

            ExprKind::Lambda { params, body } => Ok(Value::Fn(FnValue {
                name: None,
                params: params.clone(),
                body: FnBody::Ast(vec![Stmt {
                    kind: StmtKind::Return(Some(*body.clone())),
                    span: body.span.clone(),
                }]),
                closure: self.env.clone(),
            })),

            ExprKind::If { cond, then, else_ } => {
                let c = self.eval_expr(cond)?;
                if self.is_truthy(&c) {
                    self.exec_block(then)
                } else if let Some(e) = else_ {
                    self.eval_expr(e)
                } else {
                    Ok(Value::Unit)
                }
            }

            ExprKind::Block(block) => self.exec_block(block),

            ExprKind::Match { scrutinee, arms } => {
                let val = self.eval_expr(scrutinee)?;
                for arm in arms {
                    if let Some(bindings) = self.match_pattern(&arm.pattern, &val) {
                        self.env.push();
                        for (name, v) in bindings {
                            self.env.define(&name, v);
                        }
                        let result = self.eval_expr(&arm.body);
                        self.env.pop();
                        return result;
                    }
                }
                Err(InterpError::runtime("non-exhaustive match"))
            }

            ExprKind::Await(expr) => {
                // In the interpreter, await is a no-op (synchronous execution)
                self.eval_expr(expr)
            }

            ExprKind::Borrow(expr) | ExprKind::Move(expr) => {
                // In the interpreter, borrow/move are no-ops (no ownership tracking)
                self.eval_expr(expr)
            }

            ExprKind::StructLit { name, fields } => {
                let mut field_map = std::collections::HashMap::new();
                for (fname, fexpr) in fields {
                    let val = self.eval_expr(fexpr)?;
                    field_map.insert(fname.clone(), val);
                }
                Ok(Value::Struct(name.clone(), field_map))
            }
        }
    }

    // -- binary operations ----------------------------------------------------

    fn eval_binop(&mut self, op: &BinOp, lhs: &Expr, rhs: &Expr) -> InterpResult<Value> {
        let l = self.eval_expr(lhs)?;
        let r = self.eval_expr(rhs)?;
        match op {
            BinOp::Add => match (&l, &r) {
                (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a + b)),
                (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
                (Value::Int(a), Value::Float(b)) => Ok(Value::Float(*a as f64 + b)),
                (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a + *b as f64)),
                (Value::Str(a), Value::Str(b)) => Ok(Value::Str(format!("{a}{b}"))),
                _ => Err(InterpError::runtime(format!("cannot add {l} and {r}"))),
            },
            BinOp::Sub => match (&l, &r) {
                (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a - b)),
                (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a - b)),
                (Value::Int(a), Value::Float(b)) => Ok(Value::Float(*a as f64 - b)),
                (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a - *b as f64)),
                _ => Err(InterpError::runtime(format!("cannot subtract {l} and {r}"))),
            },
            BinOp::Mul => match (&l, &r) {
                (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a * b)),
                (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a * b)),
                (Value::Int(a), Value::Float(b)) => Ok(Value::Float(*a as f64 * b)),
                (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a * *b as f64)),
                _ => Err(InterpError::runtime(format!("cannot multiply {l} and {r}"))),
            },
            BinOp::Div => match (&l, &r) {
                (Value::Int(a), Value::Int(b)) => {
                    if *b == 0 {
                        return Err(InterpError::runtime("division by zero"));
                    }
                    Ok(Value::Int(a / b))
                }
                (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a / b)),
                (Value::Int(a), Value::Float(b)) => Ok(Value::Float(*a as f64 / b)),
                (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a / *b as f64)),
                _ => Err(InterpError::runtime("cannot divide")),
            },
            BinOp::Mod => match (&l, &r) {
                (Value::Int(a), Value::Int(b)) => {
                    if *b == 0 {
                        return Err(InterpError::runtime("modulo by zero"));
                    }
                    Ok(Value::Int(a % b))
                }
                _ => Err(InterpError::runtime("modulo requires integers")),
            },
            BinOp::Eq => Ok(Value::Bool(l == r)),
            BinOp::NotEq => Ok(Value::Bool(l != r)),
            BinOp::Lt => self
                .cmp_values(&l, &r)
                .map(|o| Value::Bool(o == std::cmp::Ordering::Less)),
            BinOp::Gt => self
                .cmp_values(&l, &r)
                .map(|o| Value::Bool(o == std::cmp::Ordering::Greater)),
            BinOp::LtEq => self
                .cmp_values(&l, &r)
                .map(|o| Value::Bool(o != std::cmp::Ordering::Greater)),
            BinOp::GtEq => self
                .cmp_values(&l, &r)
                .map(|o| Value::Bool(o != std::cmp::Ordering::Less)),
            BinOp::And => Ok(Value::Bool(self.is_truthy(&l) && self.is_truthy(&r))),
            BinOp::Or => Ok(Value::Bool(self.is_truthy(&l) || self.is_truthy(&r))),
        }
    }

    fn cmp_values(&self, a: &Value, b: &Value) -> InterpResult<std::cmp::Ordering> {
        match (a, b) {
            (Value::Int(x), Value::Int(y)) => Ok(x.cmp(y)),
            (Value::Float(x), Value::Float(y)) => {
                Ok(x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal))
            }
            (Value::Str(x), Value::Str(y)) => Ok(x.cmp(y)),
            (Value::Int(x), Value::Float(y)) => Ok((*x as f64)
                .partial_cmp(y)
                .unwrap_or(std::cmp::Ordering::Equal)),
            (Value::Float(x), Value::Int(y)) => Ok(x
                .partial_cmp(&(*y as f64))
                .unwrap_or(std::cmp::Ordering::Equal)),
            _ => Err(InterpError::runtime(format!("cannot compare {a} and {b}"))),
        }
    }

    // -- field access ---------------------------------------------------------

    fn eval_field(&mut self, val: Value, field: &str) -> InterpResult<Value> {
        match &val {
            Value::Str(s) => match field {
                "len" => Ok(Value::Int(s.len() as i64)),
                "upper" => Ok(Value::Str(s.to_uppercase())),
                "lower" => Ok(Value::Str(s.to_lowercase())),
                "trim" => Ok(Value::Str(s.trim().to_string())),
                _ => self.make_method(val.clone(), field),
            },
            Value::List(items) => match field {
                "len" => Ok(Value::Int(items.len() as i64)),
                "first" => Ok(items
                    .first()
                    .cloned()
                    .map(|v| Value::Option(Some(Box::new(v))))
                    .unwrap_or(Value::Option(None))),
                "last" => Ok(items
                    .last()
                    .cloned()
                    .map(|v| Value::Option(Some(Box::new(v))))
                    .unwrap_or(Value::Option(None))),
                "reverse" => {
                    let mut v = items.clone();
                    v.reverse();
                    Ok(Value::List(v))
                }
                _ => self.make_method(val.clone(), field),
            },
            Value::Struct(_, fields) => fields
                .get(field)
                .cloned()
                .ok_or_else(|| InterpError::runtime(format!("no field '{field}'"))),
            _ => Err(InterpError::runtime(format!("no field '{field}' on {val}"))),
        }
    }

    fn make_method(&self, val: Value, method: &str) -> InterpResult<Value> {
        // Returns a partially applied function with val as receiver
        let method = method.to_string();
        let val_clone = val.clone();
        Ok(Value::Fn(FnValue {
            name: Some(method.clone()),
            params: vec![],
            body: FnBody::Native(std::sync::Arc::new(move |_args| {
                Err(InterpError::runtime(format!(
                    "method '{method}' on {val_clone} not yet implemented"
                )))
            })),
            closure: Env::default(),
        }))
    }

    // -- method dispatch ------------------------------------------------------

    fn call_method(
        &mut self,
        receiver: Value,
        method: &str,
        args: &mut Vec<Value>,
    ) -> InterpResult<Value> {
        match (&receiver, method) {
            // -- String methods --
            (Value::Str(s), "len") => Ok(Value::Int(s.len() as i64)),
            (Value::Str(s), "upper") => Ok(Value::Str(s.to_uppercase())),
            (Value::Str(s), "lower") => Ok(Value::Str(s.to_lowercase())),
            (Value::Str(s), "trim") => Ok(Value::Str(s.trim().to_string())),
            (Value::Str(s), "split") => {
                let sep = match args.first() {
                    Some(Value::Str(d)) => d.clone(),
                    _ => return Err(InterpError::runtime("split() requires a string separator")),
                };
                Ok(Value::List(
                    s.split(&sep as &str)
                        .map(|p| Value::Str(p.to_string()))
                        .collect(),
                ))
            }
            (Value::Str(s), "contains") => {
                let sub = match args.first() {
                    Some(Value::Str(d)) => d.clone(),
                    _ => return Err(InterpError::runtime("contains() requires a string")),
                };
                Ok(Value::Bool(s.contains(&sub as &str)))
            }
            (Value::Str(s), "starts") => {
                let pre = match args.first() {
                    Some(Value::Str(d)) => d.clone(),
                    _ => return Err(InterpError::runtime("starts() requires a string")),
                };
                Ok(Value::Bool(s.starts_with(&pre as &str)))
            }
            (Value::Str(s), "ends") => {
                let suf = match args.first() {
                    Some(Value::Str(d)) => d.clone(),
                    _ => return Err(InterpError::runtime("ends() requires a string")),
                };
                Ok(Value::Bool(s.ends_with(&suf as &str)))
            }
            (Value::Str(s), "replace") => {
                if args.len() < 2 {
                    return Err(InterpError::runtime("replace() requires 2 args"));
                }
                let (from, to) = match (&args[0], &args[1]) {
                    (Value::Str(a), Value::Str(b)) => (a.clone(), b.clone()),
                    _ => return Err(InterpError::runtime("replace() requires string args")),
                };
                Ok(Value::Str(s.replace(&from as &str, &to as &str)))
            }
            (Value::Str(s), "find") => {
                let sub = match args.first() {
                    Some(Value::Str(d)) => d.clone(),
                    _ => return Err(InterpError::runtime("find() requires a string")),
                };
                Ok(match s.find(&sub as &str) {
                    Some(i) => Value::Option(Some(Box::new(Value::Int(i as i64)))),
                    None => Value::Option(None),
                })
            }
            // -- List methods --
            (Value::List(items), "len") => Ok(Value::Int(items.len() as i64)),
            (Value::List(items), "first") => Ok(items
                .first()
                .cloned()
                .map(|v| Value::Option(Some(Box::new(v))))
                .unwrap_or(Value::Option(None))),
            (Value::List(items), "last") => Ok(items
                .last()
                .cloned()
                .map(|v| Value::Option(Some(Box::new(v))))
                .unwrap_or(Value::Option(None))),
            (Value::List(items), "reverse") => {
                let mut v = items.clone();
                v.reverse();
                Ok(Value::List(v))
            }
            (Value::List(items), "contains") => {
                let target = args
                    .first()
                    .ok_or_else(|| InterpError::runtime("contains() requires an arg"))?;
                Ok(Value::Bool(items.contains(target)))
            }
            (Value::List(_), "push") => {
                let val = args
                    .first()
                    .cloned()
                    .ok_or_else(|| InterpError::runtime("push() requires an arg"))?;
                if let Value::List(ref mut items) = receiver.clone() {
                    let mut new_items = items.clone();
                    new_items.push(val);
                    Ok(Value::List(new_items))
                } else {
                    unreachable!()
                }
            }
            (Value::List(items), "sort") => {
                let mut v = items.clone();
                if args.is_empty() {
                    v.sort_by(|a, b| match (a, b) {
                        (Value::Int(x), Value::Int(y)) => x.cmp(y),
                        (Value::Float(x), Value::Float(y)) => {
                            x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal)
                        }
                        (Value::Str(x), Value::Str(y)) => x.cmp(y),
                        _ => std::cmp::Ordering::Equal,
                    });
                    Ok(Value::List(v))
                } else {
                    // sort with key fn
                    let key_fn = args[0].clone();
                    let mut pairs: Vec<(Value, Value)> = v
                        .into_iter()
                        .map(|item| {
                            let k = self.call_fn(key_fn.clone(), vec![item.clone()]);
                            k.map(|key| (item, key))
                        })
                        .collect::<InterpResult<_>>()?;
                    pairs.sort_by(|(_, a), (_, b)| match (a, b) {
                        (Value::Int(x), Value::Int(y)) => x.cmp(y),
                        (Value::Float(x), Value::Float(y)) => {
                            x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal)
                        }
                        (Value::Str(x), Value::Str(y)) => x.cmp(y),
                        _ => std::cmp::Ordering::Equal,
                    });
                    Ok(Value::List(
                        pairs.into_iter().map(|(item, _)| item).collect(),
                    ))
                }
            }
            (Value::List(items), "filter") => {
                let f = args
                    .first()
                    .cloned()
                    .ok_or_else(|| InterpError::runtime("filter() requires a fn arg"))?;
                let items = items.clone();
                let mut result = vec![];
                for item in items {
                    let keep = self.call_fn(f.clone(), vec![item.clone()])?;
                    if self.is_truthy(&keep) {
                        result.push(item);
                    }
                }
                Ok(Value::List(result))
            }
            (Value::List(items), "map") => {
                let f = args
                    .first()
                    .cloned()
                    .ok_or_else(|| InterpError::runtime("map() requires a fn arg"))?;
                let items = items.clone();
                let mut result = vec![];
                for item in items {
                    result.push(self.call_fn(f.clone(), vec![item])?);
                }
                Ok(Value::List(result))
            }
            (Value::List(items), "reduce") => {
                if args.len() < 2 {
                    return Err(InterpError::runtime("reduce() requires (fn, init)"));
                }
                let f = args[0].clone();
                let mut acc = args[1].clone();
                for item in items.clone() {
                    acc = self.call_fn(f.clone(), vec![acc, item])?;
                }
                Ok(acc)
            }
            (Value::List(items), "join") => {
                let sep = match args.first() {
                    Some(Value::Str(s)) => s.clone(),
                    _ => return Err(InterpError::runtime("join() requires a string separator")),
                };
                Ok(Value::Str(
                    items
                        .iter()
                        .map(|v| v.to_string())
                        .collect::<Vec<_>>()
                        .join(&sep),
                ))
            }
            (Value::List(items), "set") => {
                if args.len() < 2 {
                    return Err(InterpError::runtime("set() requires index and value"));
                }
                let idx = match &args[0] {
                    Value::Int(n) => *n as usize,
                    _ => return Err(InterpError::runtime("set() index must be int")),
                };
                let mut new_items = items.clone();
                if idx >= new_items.len() {
                    return Err(InterpError::runtime(format!(
                        "set() index {idx} out of bounds"
                    )));
                }
                new_items[idx] = args[1].clone();
                Ok(Value::List(new_items))
            }
            // -- Map methods --
            (Value::Map(pairs), "get") => {
                let key = args
                    .first()
                    .ok_or_else(|| InterpError::runtime("get() requires a key"))?;
                Ok(pairs
                    .iter()
                    .find(|(k, _)| k == key)
                    .map(|(_, v)| Value::Option(Some(Box::new(v.clone()))))
                    .unwrap_or(Value::Option(None)))
            }
            (Value::Map(pairs), "has") => {
                let key = args
                    .first()
                    .ok_or_else(|| InterpError::runtime("has() requires a key"))?;
                Ok(Value::Bool(pairs.iter().any(|(k, _)| k == key)))
            }
            (Value::Map(pairs), "keys") => {
                Ok(Value::List(pairs.iter().map(|(k, _)| k.clone()).collect()))
            }
            (Value::Map(pairs), "vals") => {
                Ok(Value::List(pairs.iter().map(|(_, v)| v.clone()).collect()))
            }
            (Value::Map(pairs), "len") => Ok(Value::Int(pairs.len() as i64)),
            (Value::Map(pairs), "set") => {
                if args.len() < 2 {
                    return Err(InterpError::runtime("map.set() requires key and value"));
                }
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
            (Value::Map(pairs), "del") => {
                let key = args
                    .first()
                    .ok_or_else(|| InterpError::runtime("del() requires key"))?
                    .clone();
                Ok(Value::Map(
                    pairs.iter().filter(|(k, _)| k != &key).cloned().collect(),
                ))
            }
            // -- Fallback: look up method as a function in env --
            _ => {
                let fn_val = self.env.get(method).ok_or_else(|| {
                    InterpError::runtime(format!("no method '{}' on {}", method, receiver))
                })?;
                let mut full_args = vec![receiver];
                full_args.append(args);
                self.call_fn(fn_val, full_args)
            }
        }
    }

    // -- function call --------------------------------------------------------

    fn call_fn(&mut self, func: Value, args: Vec<Value>) -> InterpResult<Value> {
        match func {
            Value::Fn(fv) => {
                // Clone fv upfront so we can use it freely without borrow issues
                let fv = fv.clone();
                match &fv.body {
                    FnBody::Native(f) => {
                        // Handle special higher-order builtins
                        match fv.name.as_deref() {
                            Some("filter") => return self.builtin_filter(args),
                            Some("map") => return self.builtin_map(args),
                            Some("reduce") => return self.builtin_reduce(args),
                            Some("any") => return self.builtin_any(args),
                            Some("all") => return self.builtin_all(args),
                            _ => {}
                        }
                        f(&args)
                    }
                    FnBody::Ast(stmts) => {
                        // Swap in the closure's captured locals, saving the caller's.
                        // Globals (Arc<Mutex>) are shared — mutations visible everywhere.
                        let saved_locals =
                            std::mem::replace(&mut self.env.locals, fv.closure.locals.clone());
                        self.env.push();

                        // Make function available by name for recursion
                        if let Some(ref name) = fv.name {
                            self.env.define(name, Value::Fn(fv.clone()));
                        }

                        // Bind params
                        for (i, param) in fv.params.iter().enumerate() {
                            let val = args.get(i).cloned().unwrap_or(Value::Unit);
                            self.env.define(&param.name, val);
                        }

                        let mut result = Value::Unit;
                        let mut returned = false;
                        for stmt in stmts {
                            match self.exec_stmt(stmt) {
                                Ok(v) => result = v,
                                Err(e) if e.kind == ErrorKind::Return => {
                                    result = self.return_value.take().unwrap_or(Value::Unit);
                                    returned = true;
                                    break;
                                }
                                Err(e) if e.kind == ErrorKind::Propagated => {
                                    self.env.pop();
                                    self.env.locals = saved_locals;
                                    return Ok(Value::Err(Box::new(Value::Str(e.msg))));
                                }
                                Err(e) => {
                                    self.env.pop();
                                    self.env.locals = saved_locals;
                                    return Err(e);
                                }
                            }
                        }
                        let _ = returned;
                        self.env.pop();
                        // Restore caller's local frames. Globals were mutated in-place via Rc.
                        self.env.locals = saved_locals;
                        Ok(result)
                    }
                }
            }
            other => Err(InterpError::runtime(format!("'{other}' is not callable"))),
        }
    }

    // Handle filter with a higher-order fn passed from AST context
    fn builtin_filter(&mut self, args: Vec<Value>) -> InterpResult<Value> {
        if args.len() != 2 {
            return Err(InterpError::runtime("filter() requires (list, fn)"));
        }
        let items = match &args[0] {
            Value::List(l) => l.clone(),
            _ => return Err(InterpError::runtime("filter() first arg must be a list")),
        };
        let f = args[1].clone();
        let mut result = vec![];
        for item in items {
            let keep = self.call_fn(f.clone(), vec![item.clone()])?;
            if self.is_truthy(&keep) {
                result.push(item);
            }
        }
        Ok(Value::List(result))
    }

    fn builtin_map(&mut self, args: Vec<Value>) -> InterpResult<Value> {
        if args.len() != 2 {
            return Err(InterpError::runtime("map() requires (list, fn)"));
        }
        let items = match &args[0] {
            Value::List(l) => l.clone(),
            _ => return Err(InterpError::runtime("map() first arg must be a list")),
        };
        let f = args[1].clone();
        let mut result = vec![];
        for item in items {
            result.push(self.call_fn(f.clone(), vec![item])?);
        }
        Ok(Value::List(result))
    }

    fn builtin_reduce(&mut self, args: Vec<Value>) -> InterpResult<Value> {
        if args.len() < 3 {
            return Err(InterpError::runtime("reduce() requires (list, fn, init)"));
        }
        let items = match &args[0] {
            Value::List(l) => l.clone(),
            _ => return Err(InterpError::runtime("reduce() first arg must be a list")),
        };
        let f = args[1].clone();
        let mut acc = args[2].clone();
        for item in items {
            acc = self.call_fn(f.clone(), vec![acc, item])?;
        }
        Ok(acc)
    }

    fn builtin_any(&mut self, args: Vec<Value>) -> InterpResult<Value> {
        if args.len() != 2 {
            return Err(InterpError::runtime("any() requires (list, fn)"));
        }
        let items = match &args[0] {
            Value::List(l) => l.clone(),
            _ => return Err(InterpError::runtime("any() first arg must be a list")),
        };
        let f = args[1].clone();
        for item in items {
            let result = self.call_fn(f.clone(), vec![item])?;
            if self.is_truthy(&result) {
                return Ok(Value::Bool(true));
            }
        }
        Ok(Value::Bool(false))
    }

    fn builtin_all(&mut self, args: Vec<Value>) -> InterpResult<Value> {
        if args.len() != 2 {
            return Err(InterpError::runtime("all() requires (list, fn)"));
        }
        let items = match &args[0] {
            Value::List(l) => l.clone(),
            _ => return Err(InterpError::runtime("all() first arg must be a list")),
        };
        let f = args[1].clone();
        for item in items {
            let result = self.call_fn(f.clone(), vec![item])?;
            if !self.is_truthy(&result) {
                return Ok(Value::Bool(false));
            }
        }
        Ok(Value::Bool(true))
    }

    // -- pattern matching -----------------------------------------------------

    fn match_pattern(&self, pattern: &Pattern, val: &Value) -> Option<Vec<(String, Value)>> {
        match pattern {
            Pattern::Wildcard => Some(vec![]),
            Pattern::Ident(name) => Some(vec![(name.clone(), val.clone())]),
            Pattern::Literal(lit) => {
                let matches = match (lit, val) {
                    (LitPattern::Int(a), Value::Int(b)) => a == b,
                    (LitPattern::Float(a), Value::Float(b)) => a == b,
                    (LitPattern::Str(a), Value::Str(b)) => a == b,
                    (LitPattern::Bool(a), Value::Bool(b)) => a == b,
                    _ => false,
                };
                if matches {
                    Some(vec![])
                } else {
                    None
                }
            }
            Pattern::Variant(name, inner_pats) => match val {
                Value::Variant(vname, fields) if vname == name => {
                    if inner_pats.len() != fields.len() {
                        return None;
                    }
                    let mut bindings = vec![];
                    for (pat, field) in inner_pats.iter().zip(fields.iter()) {
                        bindings.extend(self.match_pattern(pat, field)?);
                    }
                    Some(bindings)
                }
                Value::Ok(v) if name == "Ok" => {
                    if inner_pats.len() == 1 {
                        self.match_pattern(&inner_pats[0], v)
                    } else {
                        None
                    }
                }
                Value::Err(v) if name == "Err" => {
                    if inner_pats.len() == 1 {
                        self.match_pattern(&inner_pats[0], v)
                    } else {
                        None
                    }
                }
                Value::Option(Some(v)) if name == "Some" => {
                    if inner_pats.len() == 1 {
                        self.match_pattern(&inner_pats[0], v)
                    } else {
                        None
                    }
                }
                Value::Option(None) if name == "None" => Some(vec![]),
                _ => None,
            },
            Pattern::Tuple(pats) => {
                if let Value::Tuple(fields) = val {
                    if pats.len() != fields.len() {
                        return None;
                    }
                    let mut bindings = vec![];
                    for (pat, field) in pats.iter().zip(fields.iter()) {
                        bindings.extend(self.match_pattern(pat, field)?);
                    }
                    Some(bindings)
                } else {
                    None
                }
            }
            Pattern::Struct(name, field_pats) => {
                if let Value::Struct(sname, fields) = val {
                    if sname != name {
                        return None;
                    }
                    let mut bindings = vec![];
                    for (fname, pat) in field_pats {
                        let fval = fields.get(fname)?;
                        bindings.extend(self.match_pattern(pat, fval)?);
                    }
                    Some(bindings)
                } else {
                    None
                }
            }
        }
    }

    // -- helpers --------------------------------------------------------------

    fn is_truthy(&self, val: &Value) -> bool {
        match val {
            Value::Bool(b) => *b,
            Value::Int(n) => *n != 0,
            Value::Float(f) => *f != 0.0,
            Value::Str(s) => !s.is_empty(),
            Value::Unit => false,
            Value::Option(None) => false,
            Value::Option(_) => true,
            Value::List(l) => !l.is_empty(),
            _ => true,
        }
    }

    fn to_iter(&self, val: Value) -> InterpResult<Vec<Value>> {
        match val {
            Value::List(items) => Ok(items),
            Value::Str(s) => Ok(s.chars().map(|c| Value::Str(c.to_string())).collect()),
            _ => Err(InterpError::runtime(format!("cannot iterate over {val}"))),
        }
    }

    /// Resolve string interpolation: "hello {name}" → "hello Lorenzo"
    fn interpolate(&mut self, s: &str) -> InterpResult<String> {
        let mut result = String::new();
        let mut chars = s.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '{' {
                let mut expr_src = String::new();
                for inner in chars.by_ref() {
                    if inner == '}' {
                        break;
                    }
                    expr_src.push(inner);
                }
                let expr_src = expr_src.trim().to_string();

                // Empty braces {} — leave as-is for fmt() positional placeholder
                if expr_src.is_empty() {
                    result.push('{');
                    result.push('}');
                // Fast path: plain identifier — no need to lex/parse
                } else if expr_src.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    let val = self.env.get(&expr_src).ok_or_else(|| {
                        InterpError::runtime(format!(
                            "undefined variable '{expr_src}' in string interpolation"
                        ))
                    })?;
                    result.push_str(&val.to_string());
                } else {
                    // Arbitrary expression: lex + parse + eval
                    let tokens = ash_lexer::Lexer::new(&expr_src).tokenize().map_err(|e| {
                        InterpError::runtime(format!("interpolation lex error: {e}"))
                    })?;
                    let program = ash_parser::parse(tokens).map_err(|e| {
                        InterpError::runtime(format!("interpolation parse error: {}", e.msg))
                    })?;
                    // Eval as a single expression statement
                    if let Some(stmt) = program.stmts.first() {
                        if let ash_parser::ast::StmtKind::Expr(expr) = &stmt.kind {
                            let expr = expr.clone();
                            let val = self.eval_expr(&expr)?;
                            result.push_str(&val.to_string());
                        } else {
                            return Err(InterpError::runtime(format!(
                                "interpolation '{expr_src}' is not an expression"
                            )));
                        }
                    }
                }
            } else {
                result.push(c);
            }
        }
        Ok(result)
    }
}

// --- Public API --------------------------------------------------------------

pub fn run(program: &Program) -> InterpResult<Value> {
    Interpreter::new().run(program)
}

// --- Tests --------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use ash_lexer::Lexer;
    use ash_parser::parse;

    fn eval(src: &str) -> Value {
        let tokens = Lexer::new(src).tokenize().expect("lex failed");
        let program = parse(tokens).expect("parse failed");
        run(&program).expect("runtime error")
    }

    fn eval_err(src: &str) -> String {
        let tokens = Lexer::new(src).tokenize().expect("lex failed");
        let program = parse(tokens).expect("parse failed");
        run(&program).unwrap_err().msg
    }

    #[test]
    fn test_integer_arithmetic() {
        assert_eq!(eval("2 + 3"), Value::Int(5));
        assert_eq!(eval("10 - 4"), Value::Int(6));
        assert_eq!(eval("3 * 4"), Value::Int(12));
        assert_eq!(eval("10 / 2"), Value::Int(5));
        assert_eq!(eval("7 % 3"), Value::Int(1));
    }

    #[test]
    fn test_float_arithmetic() {
        assert_eq!(eval("1.5 + 2.5"), Value::Float(4.0));
        assert_eq!(eval("3.0 * 2.0"), Value::Float(6.0));
    }

    #[test]
    fn test_mixed_arithmetic() {
        assert_eq!(eval("1 + 2.5"), Value::Float(3.5));
    }

    #[test]
    fn test_string_concat() {
        assert_eq!(
            eval("\"hello\" + \" world\""),
            Value::Str("hello world".into())
        );
    }

    #[test]
    fn test_comparison() {
        assert_eq!(eval("1 < 2"), Value::Bool(true));
        assert_eq!(eval("2 > 3"), Value::Bool(false));
        assert_eq!(eval("3 == 3"), Value::Bool(true));
        assert_eq!(eval("3 != 4"), Value::Bool(true));
    }

    #[test]
    fn test_variable_binding() {
        assert_eq!(eval("x = 42\nx"), Value::Int(42));
    }

    #[test]
    fn test_let_binding() {
        assert_eq!(eval("let x = 10\nx * 2"), Value::Int(20));
    }

    #[test]
    fn test_mut_rebinding() {
        assert_eq!(eval("mut x = 1\nx = x + 1\nx"), Value::Int(2));
    }

    #[test]
    fn test_fn_def_and_call() {
        let src = "fn add(a b)\n    a + b\nadd(3 4)";
        assert_eq!(eval(src), Value::Int(7));
    }

    #[test]
    fn test_fn_return() {
        let src = "fn double(x)\n    return x * 2\ndouble(5)";
        assert_eq!(eval(src), Value::Int(10));
    }

    #[test]
    fn test_recursive_fn() {
        let src =
            "fn fact(n)\n    if n <= 1\n        1\n    else\n        n * fact(n - 1)\nfact(5)";
        assert_eq!(eval(src), Value::Int(120));
    }

    #[test]
    fn test_if_expr_true() {
        let src = "if true\n    42\nelse\n    0";
        assert_eq!(eval(src), Value::Int(42));
    }

    #[test]
    fn test_if_expr_false() {
        let src = "if false\n    42\nelse\n    0";
        assert_eq!(eval(src), Value::Int(0));
    }

    #[test]
    fn test_while_loop() {
        let src = "mut x = 0\nwhile x < 5\n    x = x + 1\nx";
        assert_eq!(eval(src), Value::Int(5));
    }

    #[test]
    fn test_for_loop() {
        let src = "mut sum = 0\nfor i in [1, 2, 3, 4, 5]\n    sum = sum + i\nsum";
        assert_eq!(eval(src), Value::Int(15));
    }

    #[test]
    fn test_lambda() {
        let src = "f = x => x * 2\nf(5)";
        assert_eq!(eval(src), Value::Int(10));
    }

    #[test]
    fn test_multi_param_lambda() {
        let src = "f = (x y) => x + y\nf(3 4)";
        assert_eq!(eval(src), Value::Int(7));
    }

    #[test]
    fn test_pipeline() {
        let src = "fn double(x)\n    x * 2\n5 |> double";
        assert_eq!(eval(src), Value::Int(10));
    }

    #[test]
    fn test_null_coalesce_some() {
        // Direct value ?? fallback  =>  direct value
        let src = "x = 42\nx ?? 0";
        assert_eq!(eval(src), Value::Int(42));
    }

    #[test]
    fn test_match_variant() {
        let src = "type Color = Red | Green | Blue\nred = Red()\nmatch red\n    Red() => 1\n    Green() => 2\n    Blue() => 3";
        assert_eq!(eval(src), Value::Int(1));
    }

    #[test]
    fn test_match_literal() {
        let src = "x = 2\nmatch x\n    1 => \"one\"\n    2 => \"two\"\n    _ => \"other\"";
        assert_eq!(eval(src), Value::Str("two".into()));
    }

    #[test]
    fn test_list_literal() {
        assert_eq!(
            eval("[1, 2, 3]"),
            Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)])
        );
    }

    #[test]
    fn test_list_len_method() {
        assert_eq!(eval("[1, 2, 3].len()"), Value::Int(3));
    }

    #[test]
    fn test_string_len_method() {
        assert_eq!(eval("\"hello\".len()"), Value::Int(5));
    }

    #[test]
    fn test_string_upper_method() {
        assert_eq!(eval("\"hello\".upper()"), Value::Str("HELLO".into()));
    }

    #[test]
    fn test_string_lower_method() {
        assert_eq!(eval("\"HELLO\".lower()"), Value::Str("hello".into()));
    }

    #[test]
    fn test_string_trim_method() {
        assert_eq!(eval("\"  hi  \".trim()"), Value::Str("hi".into()));
    }

    #[test]
    fn test_filter_builtin() {
        let src = "filter([1, 2, 3, 4], x => x > 2)";
        assert_eq!(eval(src), Value::List(vec![Value::Int(3), Value::Int(4)]));
    }

    #[test]
    fn test_map_builtin() {
        let src = "map([1, 2, 3], x => x * 2)";
        assert_eq!(
            eval(src),
            Value::List(vec![Value::Int(2), Value::Int(4), Value::Int(6)])
        );
    }

    #[test]
    fn test_string_interpolation() {
        let src = "name = \"Lorenzo\"\n\"hello {name}\"";
        assert_eq!(eval(src), Value::Str("hello Lorenzo".into()));
    }

    #[test]
    fn test_division_by_zero() {
        assert!(eval_err("1 / 0").contains("zero"));
    }

    #[test]
    fn test_undefined_variable() {
        assert!(eval_err("x + 1").contains("undefined"));
    }

    #[test]
    fn test_negation() {
        assert_eq!(eval("-5"), Value::Int(-5));
        assert_eq!(eval("-(3 + 2)"), Value::Int(-5));
    }

    #[test]
    fn test_boolean_logic() {
        assert_eq!(eval("true && false"), Value::Bool(false));
        assert_eq!(eval("true || false"), Value::Bool(true));
        assert_eq!(eval("!true"), Value::Bool(false));
    }

    #[test]
    fn test_int_conversion() {
        assert_eq!(eval("int(3.7)"), Value::Int(3));
        assert_eq!(eval("int(\"42\")"), Value::Int(42));
    }

    #[test]
    fn test_float_conversion() {
        assert_eq!(eval("float(3)"), Value::Float(3.0));
    }

    #[test]
    fn test_str_conversion() {
        assert_eq!(eval("str(42)"), Value::Str("42".into()));
    }

    #[test]
    fn test_min_max() {
        assert_eq!(eval("min(3 7)"), Value::Int(3));
        assert_eq!(eval("max(3 7)"), Value::Int(7));
    }

    #[test]
    fn test_abs() {
        assert_eq!(eval("abs(-5)"), Value::Int(5));
        assert_eq!(eval("abs(5)"), Value::Int(5));
    }

    #[test]
    fn test_closure_capture() {
        let src = "fn make_adder(n)\n    x => x + n\nadd5 = make_adder(5)\nadd5(3)";
        assert_eq!(eval(src), Value::Int(8));
    }

    #[test]
    fn test_pipeline_with_lambda() {
        let src = "[1, 2, 3, 4] |> filter(x => x > 2)";
        assert_eq!(eval(src), Value::List(vec![Value::Int(3), Value::Int(4)]));
    }
}
