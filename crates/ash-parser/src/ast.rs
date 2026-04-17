//! Ash AST
//! Defines every node type produced by the parser.
//! The AST is untyped — types are resolved later by the type checker.

use ash_lexer::Span;

// ─── Types ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum AshType {
    Int,
    Float,
    Bool,
    Str,
    Void,
    Option(Box<AshType>),               // ?T
    Result(Box<AshType>, Box<AshType>), // Result[T E]
    List(Box<AshType>),                 // [T]
    Map(Box<AshType>, Box<AshType>),    // {K: V}
    Tuple(Vec<AshType>),                // (T, U)
    Fn(Vec<AshType>, Box<AshType>),     // T => U
    Named(String),                      // User, Point …
    Generic(String),                    // T, U (single uppercase)
    Infer,                              // not yet known
}

impl std::fmt::Display for AshType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AshType::Int => write!(f, "int"),
            AshType::Float => write!(f, "float"),
            AshType::Bool => write!(f, "bool"),
            AshType::Str => write!(f, "str"),
            AshType::Void => write!(f, "void"),
            AshType::Option(t) => write!(f, "?{t}"),
            AshType::Result(t, e) => write!(f, "Result[{t} {e}]"),
            AshType::List(t) => write!(f, "[{t}]"),
            AshType::Map(k, v) => write!(f, "{{{k}: {v}}}"),
            AshType::Tuple(ts) => {
                let s: Vec<_> = ts.iter().map(|t| t.to_string()).collect();
                write!(f, "({})", s.join(" "))
            }
            AshType::Fn(args, ret) => {
                let s: Vec<_> = args.iter().map(|t| t.to_string()).collect();
                write!(f, "{} => {ret}", s.join(" "))
            }
            AshType::Named(n) => write!(f, "{n}"),
            AshType::Generic(n) => write!(f, "{n}"),
            AshType::Infer => write!(f, "_"),
        }
    }
}

// ─── Operators ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    NotEq,
    Lt,
    Gt,
    LtEq,
    GtEq,
    And,
    Or,
}

impl std::fmt::Display for BinOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Mod => "%",
            BinOp::Eq => "==",
            BinOp::NotEq => "!=",
            BinOp::Lt => "<",
            BinOp::Gt => ">",
            BinOp::LtEq => "<=",
            BinOp::GtEq => ">=",
            BinOp::And => "&&",
            BinOp::Or => "||",
        };
        write!(f, "{s}")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnOp {
    Neg, // -x
    Not, // !x
}

// ─── Expressions ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ExprKind {
    // Literals
    Int(i64),
    Float(f64),
    Str(String), // may contain {interpolation} markers
    Bool(bool),

    // Composite literals
    List(Vec<Expr>),
    Map(Vec<(Expr, Expr)>),
    Tuple(Vec<Expr>),

    // Reference
    Ident(String),

    // Operators
    BinOp {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    UnOp {
        op: UnOp,
        expr: Box<Expr>,
    },

    // Function call: callee(args)
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },

    // Method / field: obj.field  or  obj.method(args)
    Field {
        obj: Box<Expr>,
        field: String,
    },

    // Safe navigation: obj?.field
    SafeField {
        obj: Box<Expr>,
        field: String,
    },

    // Index: list[i]
    Index {
        obj: Box<Expr>,
        index: Box<Expr>,
    },

    // Pipeline: lhs |> rhs   →   rhs(lhs)
    Pipe {
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },

    // Null coalesce: lhs ?? rhs
    NullCoalesce {
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },

    // Error propagation postfix: expr!
    Propagate(Box<Expr>),

    // Range: start..end
    Range {
        start: Box<Expr>,
        end: Box<Expr>,
    },

    // Lambda: (params) => body  or  x => body
    Lambda {
        params: Vec<Param>,
        body: Box<Expr>,
    },

    // If expression (also used as statement)
    If {
        cond: Box<Expr>,
        then: Box<Block>,
        else_: Option<Box<Expr>>,
    },

    // Match expression
    Match {
        scrutinee: Box<Expr>,
        arms: Vec<MatchArm>,
    },

    // Struct literal: TypeName { field: val, field2: val2 }
    StructLit {
        name: String,
        fields: Vec<(String, Expr)>,
    },

    // Block expression: last expr is the value
    Block(Block),

    // Await: await expr
    Await(Box<Expr>),

    // Move into closure: move expr
    Move(Box<Expr>),

    // Borrow: &expr
    Borrow(Box<Expr>),
}

// ─── Statements ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum StmtKind {
    // Variable binding: x = expr  or  let x:T = expr  or  mut x = expr
    Let {
        name: String,
        ty: AshType,
        mutable: bool,
        value: Expr,
    },

    // Reassignment: target = value
    Assign {
        target: Expr,
        value: Expr,
    },

    // return expr
    Return(Option<Expr>),

    // while cond { body }
    While {
        cond: Expr,
        body: Block,
    },

    // for x in iter { body }
    For {
        var: String,
        iter: Expr,
        body: Block,
    },

    // panic "msg"
    Panic(Expr),

    // Bare expression (call, pipeline, etc.)
    Expr(Expr),

    // Function definition
    FnDef(FnDef),

    // Type definition
    TypeDef(TypeDef),

    // Module import: use "path/to/file.ash"
    Use(String),
}

// ─── Supporting structures ───────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub ty: AshType,
    pub mutable: bool,
    pub borrow: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct FnDef {
    pub name: String,
    pub generics: Vec<String>, // explicit [T U] generics
    pub params: Vec<Param>,
    pub ret: AshType,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct TypeDef {
    pub name: String,
    pub generics: Vec<String>,
    pub kind: TypeDefKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum TypeDefKind {
    // type Point\n    x:float\n    y:float
    Struct(Vec<FieldDef>),
    // type Result[T E] = Ok(T) | Err(E)
    Union(Vec<UnionVariant>),
}

#[derive(Debug, Clone)]
pub struct FieldDef {
    pub name: String,
    pub ty: AshType,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct UnionVariant {
    pub name: String,
    pub fields: Vec<AshType>, // Ok(T) → fields = [T]
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Pattern {
    Wildcard,                               // _
    Ident(String),                          // x
    Literal(LitPattern),                    // 42, "hello", true
    Variant(String, Vec<Pattern>),          // Ok(x), Err(e), Some(v)
    Tuple(Vec<Pattern>),                    // (a, b)
    Struct(String, Vec<(String, Pattern)>), // Point { x, y }
}

#[derive(Debug, Clone)]
pub enum LitPattern {
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
}

// ─── Program ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Program {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}
